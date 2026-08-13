//! IME-aware text field helpers and input element (`TypedCharacters` / `SetMarkedText`).

use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    AfterLayoutContext, AppContext, ConstrainedBox, Container, CrossAxisAlignment,
    DispatchEventResult, Element, EventContext, EventHandler, Flex, LayoutContext, PaintContext,
    ParentElement, Point, SizeConstraint, ZIndex,
};
use warpui::event::DispatchedEvent;
use warpui::fonts::FamilyId;
use warpui::keymap::Keystroke;
use warpui::{Event, View, ViewContext};

use crate::ui::clipboard::read_clipboard_text;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum TextFieldEditAction {
    TypedCharacters(String),
    SetMarkedText {
        marked_text: String,
        selected_range: Range<usize>,
    },
    ClearMarkedText,
    Backspace,
    Delete,
    ClearAll,
    /// Ends [`TextFieldState::suppress_ime_replay`] after the user presses a typing key.
    EndImeSuppress,
    InsertNewline,
    Paste(String),
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveHome,
    MoveEnd,
}

#[derive(Debug, Clone)]
pub struct TextFieldState {
    pub marked_text: String,
    pub marked_range: Range<usize>,
    /// Character index into `draft` (not byte index).
    pub cursor: usize,
    /// After [`TextFieldEditAction::ClearAll`], drop stale IME preedit/commits until the user types again.
    suppress_ime_replay: bool,
}

impl Default for TextFieldState {
    fn default() -> Self {
        Self {
            marked_text: String::new(),
            marked_range: 0..0,
            cursor: 0,
            suppress_ime_replay: false,
        }
    }
}

impl TextFieldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_marked(&mut self) {
        self.marked_text.clear();
        self.marked_range = 0..0;
    }

    /// Clamp cursor to the draft length (call after externally replacing draft).
    pub fn clamp_cursor(&mut self, draft: &str) {
        self.cursor = self.cursor.min(char_len(draft));
    }

    pub fn move_cursor_to_end(&mut self, draft: &str) {
        self.cursor = char_len(draft);
    }

    pub fn apply(&mut self, draft: &mut String, action: &TextFieldEditAction) {
        self.clamp_cursor(draft);
        match action {
            TextFieldEditAction::TypedCharacters(chars) => {
                if self.suppress_ime_replay {
                    return;
                }
                self.clear_marked();
                insert_at_char(draft, self.cursor, chars);
                self.cursor = self.cursor.saturating_add(char_len(chars));
            }
            TextFieldEditAction::SetMarkedText {
                marked_text,
                selected_range,
            } => {
                if is_spurious_replacement_text(marked_text) {
                    self.clear_marked();
                    return;
                }
                if self.suppress_ime_replay {
                    if marked_text.is_empty() {
                        self.suppress_ime_replay = false;
                    }
                    return;
                }
                self.marked_text = marked_text.clone();
                self.marked_range = selected_range.clone();
            }
            TextFieldEditAction::ClearMarkedText => {
                self.clear_marked();
                self.suppress_ime_replay = false;
            }
            TextFieldEditAction::EndImeSuppress => {
                self.suppress_ime_replay = false;
            }
            TextFieldEditAction::Backspace => {
                if !self.marked_text.is_empty() {
                    pop_char(&mut self.marked_text);
                    sync_marked_range(self);
                } else if self.cursor > 0 {
                    self.cursor = delete_char_before(draft, self.cursor);
                }
            }
            TextFieldEditAction::Delete => {
                if !self.marked_text.is_empty() {
                    self.marked_text = self.marked_text.chars().skip(1).collect();
                    sync_marked_range(self);
                } else {
                    delete_char_at(draft, self.cursor);
                }
            }
            TextFieldEditAction::ClearAll => {
                self.clear_marked();
                draft.clear();
                self.cursor = 0;
                self.suppress_ime_replay = true;
            }
            TextFieldEditAction::InsertNewline => {
                self.suppress_ime_replay = false;
                self.clear_marked();
                insert_at_char(draft, self.cursor, "\n");
                self.cursor = self.cursor.saturating_add(1);
            }
            TextFieldEditAction::Paste(text) => {
                self.suppress_ime_replay = false;
                self.clear_marked();
                let cleaned: String = text
                    .chars()
                    .filter(|ch| *ch == '\n' || !ch.is_control())
                    .collect();
                insert_at_char(draft, self.cursor, &cleaned);
                self.cursor = self.cursor.saturating_add(char_len(&cleaned));
            }
            TextFieldEditAction::MoveLeft => {
                if self.marked_text.is_empty() && self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            TextFieldEditAction::MoveRight => {
                if self.marked_text.is_empty() {
                    let len = char_len(draft);
                    if self.cursor < len {
                        self.cursor += 1;
                    }
                }
            }
            TextFieldEditAction::MoveHome => {
                if self.marked_text.is_empty() {
                    self.cursor = line_start(draft, self.cursor);
                }
            }
            TextFieldEditAction::MoveEnd => {
                if self.marked_text.is_empty() {
                    self.cursor = line_end(draft, self.cursor);
                }
            }
            TextFieldEditAction::MoveUp => {
                if self.marked_text.is_empty() {
                    self.cursor = move_vertical(draft, self.cursor, true);
                }
            }
            TextFieldEditAction::MoveDown => {
                if self.marked_text.is_empty() {
                    self.cursor = move_vertical(draft, self.cursor, false);
                }
            }
        }
        self.clamp_cursor(draft);
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn split_at_char(s: &str, idx: usize) -> (String, String) {
    let idx = idx.min(char_len(s));
    let mut chars = s.chars();
    let before: String = chars.by_ref().take(idx).collect();
    let after: String = chars.collect();
    (before, after)
}

fn insert_at_char(s: &mut String, idx: usize, insert: &str) {
    if insert.is_empty() {
        return;
    }
    let (before, after) = split_at_char(s, idx);
    *s = format!("{before}{insert}{after}");
}

fn delete_char_before(s: &mut String, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let (before, after) = split_at_char(s, idx);
    let mut new_before = before;
    pop_char(&mut new_before);
    let new_idx = char_len(&new_before);
    *s = format!("{new_before}{after}");
    new_idx
}

fn delete_char_at(s: &mut String, idx: usize) {
    let len = char_len(s);
    if idx >= len {
        return;
    }
    let (before, after) = split_at_char(s, idx);
    let mut after_chars = after.chars();
    let _ = after_chars.next();
    let rest: String = after_chars.collect();
    *s = format!("{before}{rest}");
}

fn line_start(draft: &str, cursor: usize) -> usize {
    let chars: Vec<char> = draft.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut i = cursor;
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

fn line_end(draft: &str, cursor: usize) -> usize {
    let chars: Vec<char> = draft.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut i = cursor;
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    i
}

fn move_vertical(draft: &str, cursor: usize, up: bool) -> usize {
    let chars: Vec<char> = draft.chars().collect();
    let cursor = cursor.min(chars.len());
    let start = line_start(draft, cursor);
    let col = cursor - start;
    if up {
        if start == 0 {
            return cursor;
        }
        let prev_end = start - 1;
        let prev_start = line_start(draft, prev_end);
        let prev_len = prev_end - prev_start;
        prev_start + col.min(prev_len)
    } else {
        let end = line_end(draft, cursor);
        if end >= chars.len() {
            return cursor;
        }
        let next_start = end + 1;
        let next_end = line_end(draft, next_start);
        let next_len = next_end - next_start;
        next_start + col.min(next_len)
    }
}

pub fn pop_char(draft: &mut String) {
    if draft.pop().is_some() {
        while draft.ends_with('\u{308}') || draft.ends_with('\u{300}') {
            draft.pop();
        }
    }
}

fn sync_marked_range(state: &mut TextFieldState) {
    if state.marked_text.is_empty() {
        state.marked_range = 0..0;
        return;
    }
    let len = state.marked_text.chars().count();
    state.marked_range = 0..len;
}

fn sanitized_typed_characters(chars: &str, ime_preedit: bool) -> Option<String> {
    if ime_preedit && is_spurious_replacement_text(chars) {
        return None;
    }

    let sanitized: String = chars.chars().filter(|ch| !ch.is_control()).collect();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn is_spurious_replacement_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| ch == '\u{FFFD}')
}

fn is_user_typing_keystroke(keystroke: &Keystroke) -> bool {
    if keystroke.ctrl || keystroke.meta || keystroke.alt {
        return false;
    }
    !matches!(
        keystroke.key.as_str(),
        "backspace"
            | "delete"
            | "enter"
            | "return"
            | "escape"
            | "tab"
            | "left"
            | "right"
            | "up"
            | "down"
            | "home"
            | "end"
    )
}

pub const CARET_BLINK_MS: u64 = 530;

#[derive(Debug, Clone, Default)]
pub struct CaretBlink {
    pub visible: bool,
    running: bool,
}

impl CaretBlink {
    pub fn new() -> Self {
        Self {
            visible: true,
            running: false,
        }
    }

    pub fn reset(&mut self) {
        self.visible = true;
    }

    pub fn on_blur(&mut self) {
        self.visible = true;
        self.running = false;
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn ensure_animating<V: CaretBlinkHost + 'static>(&mut self, ctx: &mut ViewContext<V>) {
        if self.running {
            return;
        }
        self.running = true;
        ctx.spawn(
            async move {
                tokio::time::sleep(Duration::from_millis(CARET_BLINK_MS)).await;
            },
            move |view, _, ctx| {
                if view.caret_input_focused() {
                    view.caret_blink().toggle();
                    ctx.notify();
                } else {
                    view.caret_blink().on_blur();
                    return;
                }
                view.caret_blink().running = false;
                if view.caret_input_focused() {
                    view.caret_blink().ensure_animating(ctx);
                }
            },
        );
    }
}

pub trait CaretBlinkHost: View {
    fn caret_blink(&mut self) -> &mut CaretBlink;
    fn caret_input_focused(&self) -> bool;
}

pub fn sync_caret_blink<V: CaretBlinkHost + 'static>(view: &mut V, ctx: &mut ViewContext<V>) {
    let focused = view.caret_input_focused();
    if focused {
        view.caret_blink().reset();
        view.caret_blink().ensure_animating(ctx);
    } else {
        view.caret_blink().on_blur();
    }
}

pub fn should_show_placeholder(focused: bool, draft: &str, marked: &str) -> bool {
    !focused && draft.is_empty() && marked.is_empty()
}

pub fn render_caret(show: bool, blink_on: bool) -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_width(2.0)
            .with_height(18.0)
            .finish(),
    )
    .with_background(if show && blink_on {
        theme::accent_cool()
    } else {
        ColorU::transparent_black()
    })
    .with_horizontal_margin(1.0)
    .finish()
}

pub fn render_field_with_caret(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    caret_blink: bool,
    cursor: usize,
) -> Box<dyn Element> {
    render_field_with_caret_sized(
        draft,
        marked,
        placeholder,
        font,
        focused,
        disabled,
        caret_blink,
        cursor,
        ui_text::BODY_SIZE,
    )
}

pub fn compose_should_show_placeholder(draft: &str, marked: &str) -> bool {
    draft.is_empty() && marked.is_empty()
}

pub fn render_compose_field_with_caret(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    caret_blink: bool,
    cursor: usize,
) -> Box<dyn Element> {
    let show_caret = focused && !disabled;
    if compose_should_show_placeholder(draft, marked) {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(
            warpui::elements::Text::new(
                placeholder.to_string(),
                font,
                ui_text::CHAT_COMPOSE_FONT_SIZE,
            )
            .with_color(theme::placeholder())
            .finish(),
        );
        if show_caret {
            row.add_child(render_caret(true, caret_blink));
        }
        return row.finish();
    }
    render_field_with_caret_sized(
        draft,
        marked,
        placeholder,
        font,
        focused,
        disabled,
        caret_blink,
        cursor,
        ui_text::CHAT_COMPOSE_FONT_SIZE,
    )
}

/// Sidebar / thread search: keep placeholder visible when focused and empty (align AI search).
pub fn render_search_field_with_caret(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    caret_blink: bool,
    cursor: usize,
) -> Box<dyn Element> {
    let show_caret = focused && !disabled;
    if compose_should_show_placeholder(draft, marked) {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(
            warpui::elements::Text::new(placeholder.to_string(), font, ui_text::BODY_SIZE)
                .with_color(theme::placeholder())
                .finish(),
        );
        if show_caret {
            row.add_child(render_caret(true, caret_blink));
        }
        return row.finish();
    }
    render_field_with_caret_sized(
        draft,
        marked,
        placeholder,
        font,
        focused,
        disabled,
        caret_blink,
        cursor,
        ui_text::BODY_SIZE,
    )
}

fn render_field_with_caret_sized(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    caret_blink: bool,
    cursor: usize,
    font_size: f32,
) -> Box<dyn Element> {
    let show_caret = focused && !disabled;
    let draft_empty = draft.is_empty() && marked.is_empty();
    if show_caret && draft_empty {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(render_caret(true, caret_blink));
        return row.finish();
    }

    if !show_caret {
        return render_field_text_sized(
            draft,
            marked,
            placeholder,
            font,
            focused,
            disabled,
            font_size,
        );
    }

    render_field_text_with_caret_sized(
        draft,
        marked,
        placeholder,
        font,
        focused,
        disabled,
        caret_blink,
        cursor,
        font_size,
    )
}

pub fn display_with_preedit(draft: &str, marked: &str, placeholder: &str) -> String {
    if !marked.is_empty() {
        if draft.is_empty() {
            return marked.to_string();
        }
        return format!("{draft}{marked}");
    }
    if draft.is_empty() {
        return placeholder.to_string();
    }
    draft.to_string()
}

pub fn render_field_text(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
) -> Box<dyn Element> {
    render_field_text_sized(
        draft,
        marked,
        placeholder,
        font,
        focused,
        disabled,
        ui_text::BODY_SIZE,
    )
}

fn field_text_el(
    text: String,
    color: pathfinder_color::ColorU,
    font: FamilyId,
    font_size: f32,
) -> Box<dyn Element> {
    warpui::elements::Text::new(text, font, font_size)
        .with_color(color)
        .finish()
}

fn render_field_text_sized(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    font_size: f32,
) -> Box<dyn Element> {
    let show_placeholder = should_show_placeholder(focused, draft, marked);
    if show_placeholder {
        return field_text_el(
            placeholder.to_string(),
            theme::placeholder(),
            font,
            font_size,
        );
    }

    let text_color = if disabled {
        theme::placeholder()
    } else {
        theme::text()
    };
    let marked_color = if focused {
        theme::accent_cool()
    } else {
        theme::muted()
    };

    if marked.is_empty() {
        return field_text_el(draft.to_string(), text_color, font, font_size);
    }

    if !draft.contains('\n') {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(warpui::elements::MainAxisSize::Min);
        if !draft.is_empty() {
            row.add_child(field_text_el(
                draft.to_string(),
                text_color,
                font,
                font_size,
            ));
        }
        row.add_child(field_text_el(
            marked.to_string(),
            marked_color,
            font,
            font_size,
        ));
        return row.finish();
    }

    let mut col = Flex::column().with_main_axis_size(warpui::elements::MainAxisSize::Min);
    let lines: Vec<&str> = draft.split('\n').collect();
    let last = lines.len().saturating_sub(1);
    for (idx, line) in lines.iter().enumerate() {
        if idx == last {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(warpui::elements::MainAxisSize::Min);
            if !line.is_empty() {
                row.add_child(field_text_el(
                    (*line).to_string(),
                    text_color,
                    font,
                    font_size,
                ));
            }
            row.add_child(field_text_el(
                marked.to_string(),
                marked_color,
                font,
                font_size,
            ));
            col.add_child(row.finish());
        } else {
            col.add_child(field_text_el(
                (*line).to_string(),
                text_color,
                font,
                font_size,
            ));
        }
    }
    col.finish()
}

fn render_field_text_with_caret_sized(
    draft: &str,
    marked: &str,
    placeholder: &str,
    font: FamilyId,
    focused: bool,
    disabled: bool,
    caret_blink: bool,
    cursor: usize,
    font_size: f32,
) -> Box<dyn Element> {
    let show_placeholder = should_show_placeholder(focused, draft, marked);
    if show_placeholder {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(field_text_el(
            placeholder.to_string(),
            theme::placeholder(),
            font,
            font_size,
        ));
        row.add_child(render_caret(true, caret_blink));
        return row.finish();
    }

    let text_color = if disabled {
        theme::placeholder()
    } else {
        theme::text()
    };
    let marked_color = if focused {
        theme::accent_cool()
    } else {
        theme::muted()
    };

    let cursor = cursor.min(char_len(draft));
    let (before, after) = split_at_char(draft, cursor);
    let prefix = if marked.is_empty() {
        before.clone()
    } else {
        format!("{before}{marked}")
    };

    if !prefix.contains('\n') && !after.contains('\n') {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(warpui::elements::MainAxisSize::Min);
        if !before.is_empty() {
            row.add_child(field_text_el(before, text_color, font, font_size));
        }
        if !marked.is_empty() {
            row.add_child(field_text_el(
                marked.to_string(),
                marked_color,
                font,
                font_size,
            ));
        }
        row.add_child(render_caret(true, caret_blink));
        if !after.is_empty() {
            row.add_child(field_text_el(after, text_color, font, font_size));
        }
        return row.finish();
    }

    // Multiline: caret on the line that contains the cursor (marked text sits on that line).
    let caret_line = prefix.chars().filter(|c| *c == '\n').count();
    let prefix_lines: Vec<&str> = prefix.split('\n').collect();
    let after_lines: Vec<&str> = after.split('\n').collect();
    let total_lines = prefix_lines.len() + after_lines.len().saturating_sub(1);

    let mut col = Flex::column().with_main_axis_size(warpui::elements::MainAxisSize::Min);
    for idx in 0..total_lines {
        if idx < caret_line {
            col.add_child(field_text_el(
                prefix_lines[idx].to_string(),
                text_color,
                font,
                font_size,
            ));
            continue;
        }
        if idx == caret_line {
            let line_before = prefix_lines.get(idx).copied().unwrap_or("");
            let line_after = after_lines.first().copied().unwrap_or("");
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(warpui::elements::MainAxisSize::Min);
            if !marked.is_empty() && line_before.ends_with(marked) {
                let draft_part = &line_before[..line_before.len().saturating_sub(marked.len())];
                if !draft_part.is_empty() {
                    row.add_child(field_text_el(
                        draft_part.to_string(),
                        text_color,
                        font,
                        font_size,
                    ));
                }
                row.add_child(field_text_el(
                    marked.to_string(),
                    marked_color,
                    font,
                    font_size,
                ));
            } else if !line_before.is_empty() {
                row.add_child(field_text_el(
                    line_before.to_string(),
                    text_color,
                    font,
                    font_size,
                ));
            }
            row.add_child(render_caret(true, caret_blink));
            if !line_after.is_empty() {
                row.add_child(field_text_el(
                    line_after.to_string(),
                    text_color,
                    font,
                    font_size,
                ));
            }
            col.add_child(row.finish());
            continue;
        }
        let after_idx = idx - caret_line;
        let line = after_lines.get(after_idx).copied().unwrap_or("");
        col.add_child(field_text_el(line.to_string(), text_color, font, font_size));
    }
    col.finish()
}

pub fn wrap_text_field_focus_on_click(
    input: Box<dyn Element>,
    on_focus: impl Fn(&mut EventContext) + 'static,
) -> Box<dyn Element> {
    wrap_text_field_focus_on_click_with_label(input, "聚焦输入框", on_focus)
}

pub fn wrap_text_field_focus_on_click_with_label(
    input: Box<dyn Element>,
    label: impl Into<String>,
    on_focus: impl Fn(&mut EventContext) + 'static,
) -> Box<dyn Element> {
    EventHandler::new(input)
        .with_automation_label(label)
        .on_left_mouse_down(move |ctx, _, _| {
            on_focus(ctx);
            DispatchEventResult::StopPropagation
        })
        .finish()
}

pub fn compose_input_height(draft: &str, marked: &str) -> f32 {
    let mut measure = draft.to_string();
    if !marked.is_empty() {
        if measure.is_empty() {
            measure = marked.to_string();
        } else if !measure.ends_with('\n') {
            measure.push_str(marked);
        } else {
            measure.push_str(marked);
        }
    }
    crate::ui::multiline_input::compose_box_height(
        &measure,
        crate::ui::multiline_input::DEFAULT_COLS,
    )
}

type EditCallback = Rc<dyn Fn(&mut EventContext, TextFieldEditAction)>;
type KeydownCallback = Rc<dyn Fn(&mut EventContext, &Keystroke) -> DispatchEventResult>;
type PasteCallback = Rc<dyn Fn(&mut EventContext) -> DispatchEventResult>;

pub struct TextFieldInput {
    child: Box<dyn Element>,
    focused: bool,
    disabled: bool,
    /// When true, Backspace/Delete are left to the platform IME (SetMarkedText updates).
    ime_preedit: bool,
    always_handle: bool,
    on_edit: EditCallback,
    on_keydown: Option<KeydownCallback>,
    on_paste: Option<PasteCallback>,
    child_max_z_index: Option<ZIndex>,
    origin: Option<Point>,
    size: Option<Vector2F>,
}

impl TextFieldInput {
    pub fn builder(
        child: Box<dyn Element>,
        on_edit: impl Fn(&mut EventContext, TextFieldEditAction) + 'static,
    ) -> Self {
        Self {
            child,
            focused: false,
            disabled: false,
            ime_preedit: false,
            always_handle: true,
            on_edit: Rc::new(on_edit),
            on_keydown: None,
            on_paste: None,
            child_max_z_index: None,
            origin: None,
            size: None,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn ime_preedit(mut self, active: bool) -> Self {
        self.ime_preedit = active;
        self
    }

    pub fn on_keydown(
        mut self,
        callback: impl Fn(&mut EventContext, &Keystroke) -> DispatchEventResult + 'static,
    ) -> Self {
        self.on_keydown = Some(Rc::new(callback));
        self
    }

    /// If the callback returns [`DispatchEventResult::StopPropagation`], the default
    /// text paste is skipped (the callback is expected to handle clipboard media).
    pub fn on_paste(
        mut self,
        callback: impl Fn(&mut EventContext) -> DispatchEventResult + 'static,
    ) -> Self {
        self.on_paste = Some(Rc::new(callback));
        self
    }

    pub fn finish(self) -> Box<dyn Element> {
        Box::new(self)
    }
}

impl Element for TextFieldInput {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let size = self.child.layout(constraint, ctx, app);
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        self.child.paint(origin, ctx, app);
        self.child_max_z_index = Some(ctx.scene.max_active_z_index());
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        let handled = self.child.dispatch_event(event, ctx, app);
        if handled && !self.always_handle {
            return true;
        }

        let Some(z_index) = self.child_max_z_index else {
            return handled;
        };

        if !self.focused || self.disabled {
            return handled;
        }

        match event.at_z_index(z_index, ctx) {
            Some(Event::TypedCharacters { chars }) if !chars.is_empty() => {
                let Some(chars) = sanitized_typed_characters(chars, self.ime_preedit) else {
                    return true;
                };
                (self.on_edit)(ctx, TextFieldEditAction::TypedCharacters(chars));
                return true;
            }
            Some(Event::SetMarkedText {
                marked_text,
                selected_range,
            }) => {
                (self.on_edit)(
                    ctx,
                    TextFieldEditAction::SetMarkedText {
                        marked_text: marked_text.clone(),
                        selected_range: selected_range.clone(),
                    },
                );
                return true;
            }
            Some(Event::ClearMarkedText) => {
                (self.on_edit)(ctx, TextFieldEditAction::ClearMarkedText);
                return true;
            }
            Some(Event::KeyDown {
                keystroke,
                is_composing,
                ..
            }) => {
                if *is_composing {
                    return false;
                }
                if keystroke.ctrl || keystroke.meta {
                    if keystroke.key.eq_ignore_ascii_case("v") {
                        if let Some(cb) = &self.on_paste {
                            if matches!(cb(ctx), DispatchEventResult::StopPropagation) {
                                return true;
                            }
                        }
                        if let Some(text) = read_clipboard_text() {
                            (self.on_edit)(ctx, TextFieldEditAction::Paste(text));
                            return true;
                        }
                    }
                    if keystroke.key.eq_ignore_ascii_case("backspace") {
                        (self.on_edit)(ctx, TextFieldEditAction::ClearAll);
                        return true;
                    }
                    return if let Some(cb) = &self.on_keydown {
                        matches!(cb(ctx, keystroke), DispatchEventResult::StopPropagation)
                    } else {
                        false
                    };
                }
                if keystroke.alt {
                    return false;
                }
                if is_user_typing_keystroke(keystroke) {
                    (self.on_edit)(ctx, TextFieldEditAction::EndImeSuppress);
                    return false;
                }
                match keystroke.key.as_str() {
                    "backspace" | "delete" => {
                        if self.ime_preedit {
                            return false;
                        }
                        let action = if keystroke.key == "delete" {
                            TextFieldEditAction::Delete
                        } else {
                            TextFieldEditAction::Backspace
                        };
                        (self.on_edit)(ctx, action);
                        return true;
                    }
                    "left" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveLeft);
                        return true;
                    }
                    "right" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveRight);
                        return true;
                    }
                    "up" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveUp);
                        return true;
                    }
                    "down" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveDown);
                        return true;
                    }
                    "home" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveHome);
                        return true;
                    }
                    "end" => {
                        (self.on_edit)(ctx, TextFieldEditAction::MoveEnd);
                        return true;
                    }
                    "enter" | "return" | "escape" | "tab" => {
                        if let Some(cb) = &self.on_keydown {
                            return matches!(
                                cb(ctx, keystroke),
                                DispatchEventResult::StopPropagation
                            );
                        }
                        return false;
                    }
                    _ => return false,
                }
            }
            _ => {}
        }

        handled
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

#[cfg(test)]
mod tests {
    use super::{
        display_with_preedit, pop_char, sanitized_typed_characters, should_show_placeholder,
        TextFieldEditAction, TextFieldState,
    };

    #[test]
    fn pop_char_removes_utf8_rune() {
        let mut draft = "你好ab".to_string();
        pop_char(&mut draft);
        assert_eq!(draft, "你好a");
        pop_char(&mut draft);
        assert_eq!(draft, "你好");
    }

    #[test]
    fn ime_commit_appends_to_draft() {
        let mut draft = String::new();
        let mut state = TextFieldState::new();
        state.apply(
            &mut draft,
            &TextFieldEditAction::SetMarkedText {
                marked_text: "ni".into(),
                selected_range: 0..2,
            },
        );
        assert_eq!(state.marked_text, "ni");
        state.apply(
            &mut draft,
            &TextFieldEditAction::TypedCharacters("你".into()),
        );
        assert_eq!(draft, "你");
        assert_eq!(state.cursor, 1);
        assert!(state.marked_text.is_empty());
    }

    #[test]
    fn display_with_preedit_shows_marked_segment() {
        let out = display_with_preedit("hello", "世界", "placeholder");
        assert_eq!(out, "hello世界");
    }

    #[test]
    fn compose_input_height_grows_with_newlines() {
        use super::compose_input_height;
        assert_eq!(compose_input_height("", ""), 22.0);
        assert!(compose_input_height("a\nb\nc", "") > 22.0);
    }

    #[test]
    fn placeholder_hidden_while_focused() {
        assert!(!should_show_placeholder(true, "", ""));
        assert!(should_show_placeholder(false, "", ""));
        assert!(!should_show_placeholder(true, "x", ""));
        assert!(!should_show_placeholder(false, "", "preedit"));
    }

    #[test]
    fn compose_placeholder_visible_while_focused() {
        use super::compose_should_show_placeholder;
        assert!(compose_should_show_placeholder("", ""));
        assert!(!compose_should_show_placeholder("x", ""));
        assert!(!compose_should_show_placeholder("", "preedit"));
    }

    #[test]
    fn backspace_shrinks_preedit_without_touching_draft() {
        let mut draft = "committed".to_string();
        let mut state = TextFieldState::new();
        state.move_cursor_to_end(&draft);
        state.apply(
            &mut draft,
            &TextFieldEditAction::SetMarkedText {
                marked_text: "nihao".into(),
                selected_range: 0..5,
            },
        );
        state.apply(&mut draft, &TextFieldEditAction::Backspace);
        assert_eq!(draft, "committed");
        assert_eq!(state.marked_text, "niha");
    }

    #[test]
    fn backspace_control_text_is_not_inserted() {
        assert_eq!(sanitized_typed_characters("\u{8}", false), None);
        assert_eq!(sanitized_typed_characters("\u{7f}", false), None);
        assert_eq!(
            sanitized_typed_characters("a\u{8}b", false),
            Some("ab".to_string())
        );
    }

    #[test]
    fn replacement_preedit_replay_is_ignored() {
        assert_eq!(sanitized_typed_characters("\u{FFFD}", true), None);

        let mut draft = "user@example.com".to_string();
        let mut state = TextFieldState::new();
        state.move_cursor_to_end(&draft);
        state.apply(
            &mut draft,
            &TextFieldEditAction::SetMarkedText {
                marked_text: "\u{FFFD}".into(),
                selected_range: 0..1,
            },
        );
        assert_eq!(draft, "user@example.com");
        assert!(state.marked_text.is_empty());
    }

    #[test]
    fn clear_all_suppresses_stale_ime_replay() {
        let mut draft = "user@example.com".to_string();
        let mut state = TextFieldState::new();
        state.apply(&mut draft, &TextFieldEditAction::ClearAll);
        assert!(draft.is_empty());
        assert_eq!(state.cursor, 0);

        state.apply(
            &mut draft,
            &TextFieldEditAction::SetMarkedText {
                marked_text: "\u{FFFD}".into(),
                selected_range: 0..3,
            },
        );
        assert!(draft.is_empty());
        assert!(state.marked_text.is_empty());

        state.apply(
            &mut draft,
            &TextFieldEditAction::TypedCharacters("\u{FFFD}".into()),
        );
        assert!(draft.is_empty());

        state.apply(&mut draft, &TextFieldEditAction::EndImeSuppress);
        state.apply(
            &mut draft,
            &TextFieldEditAction::TypedCharacters("a".into()),
        );
        assert_eq!(draft, "a");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn arrow_keys_move_cursor_through_utf8() {
        let mut draft = "你好世界".to_string();
        let mut state = TextFieldState::new();
        state.move_cursor_to_end(&draft);
        assert_eq!(state.cursor, 4);
        state.apply(&mut draft, &TextFieldEditAction::MoveLeft);
        assert_eq!(state.cursor, 3);
        state.apply(&mut draft, &TextFieldEditAction::MoveLeft);
        assert_eq!(state.cursor, 2);
        state.apply(&mut draft, &TextFieldEditAction::MoveHome);
        assert_eq!(state.cursor, 0);
        state.apply(&mut draft, &TextFieldEditAction::MoveEnd);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn insert_and_backspace_at_cursor_middle() {
        let mut draft = "你好世界".to_string();
        let mut state = TextFieldState::new();
        state.cursor = 2;
        state.apply(
            &mut draft,
            &TextFieldEditAction::TypedCharacters("中".into()),
        );
        assert_eq!(draft, "你好中世界");
        assert_eq!(state.cursor, 3);
        state.apply(&mut draft, &TextFieldEditAction::Backspace);
        assert_eq!(draft, "你好世界");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn move_up_down_across_lines() {
        let mut draft = "abc\ndef".to_string();
        let mut state = TextFieldState::new();
        state.cursor = 5; // on 'e'
        state.apply(&mut draft, &TextFieldEditAction::MoveUp);
        assert_eq!(state.cursor, 1); // 'b'
        state.apply(&mut draft, &TextFieldEditAction::MoveDown);
        assert_eq!(state.cursor, 5);
    }
}
