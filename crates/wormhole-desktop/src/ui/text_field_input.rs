//! IME-aware text field helpers and input element (`TypedCharacters` / `SetMarkedText`).

use std::ops::Range;
use std::rc::Rc;

use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    AfterLayoutContext, AppContext, CrossAxisAlignment, DispatchEventResult, Element,
    EventContext, Flex, LayoutContext, PaintContext, ParentElement, Point, SizeConstraint, ZIndex,
};
use warpui::event::DispatchedEvent;
use warpui::fonts::FamilyId;
use warpui::keymap::Keystroke;
use warpui::Event;

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
    InsertNewline,
    Paste(String),
}

#[derive(Debug, Clone, Default)]
pub struct TextFieldState {
    pub marked_text: String,
    pub marked_range: Range<usize>,
}

impl TextFieldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_marked(&mut self) {
        self.marked_text.clear();
        self.marked_range = 0..0;
    }

    pub fn apply(&mut self, draft: &mut String, action: &TextFieldEditAction) {
        match action {
            TextFieldEditAction::TypedCharacters(chars) => {
                self.clear_marked();
                draft.push_str(chars);
            }
            TextFieldEditAction::SetMarkedText {
                marked_text,
                selected_range,
            } => {
                self.marked_text = marked_text.clone();
                self.marked_range = selected_range.clone();
            }
            TextFieldEditAction::ClearMarkedText => self.clear_marked(),
            TextFieldEditAction::Backspace => {
                self.clear_marked();
                pop_char(draft);
            }
            TextFieldEditAction::InsertNewline => {
                self.clear_marked();
                draft.push('\n');
            }
            TextFieldEditAction::Paste(text) => {
                self.clear_marked();
                draft.push_str(text);
            }
        }
    }
}

pub fn pop_char(draft: &mut String) {
    if draft.pop().is_some() {
        while draft.ends_with('\u{308}') || draft.ends_with('\u{300}') {
            draft.pop();
        }
    }
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
    let show_placeholder = draft.is_empty() && marked.is_empty();
    if show_placeholder {
        return ui_text::body(placeholder.to_string(), font)
            .with_color(theme::placeholder())
            .finish();
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
        return ui_text::body(draft.to_string(), font)
            .with_color(text_color)
            .finish();
    }

    if !draft.contains('\n') {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(warpui::elements::MainAxisSize::Min);
        if !draft.is_empty() {
            row.add_child(
                ui_text::body(draft.to_string(), font)
                    .with_color(text_color)
                    .finish(),
            );
        }
        row.add_child(
            ui_text::body(marked.to_string(), font)
                .with_color(marked_color)
                .finish(),
        );
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
                row.add_child(
                    ui_text::body((*line).to_string(), font)
                        .with_color(text_color)
                        .finish(),
                );
            }
            row.add_child(
                ui_text::body(marked.to_string(), font)
                    .with_color(marked_color)
                    .finish(),
            );
            col.add_child(row.finish());
        } else {
            col.add_child(
                ui_text::body((*line).to_string(), font)
                    .with_color(text_color)
                    .finish(),
            );
        }
    }
    col.finish()
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
    crate::ui::multiline_input::box_height(&measure, crate::ui::multiline_input::DEFAULT_COLS)
}

type EditCallback = Rc<dyn Fn(&mut EventContext, TextFieldEditAction)>;
type KeydownCallback = Rc<dyn Fn(&mut EventContext, &Keystroke) -> DispatchEventResult>;

pub struct TextFieldInput {
    child: Box<dyn Element>,
    focused: bool,
    disabled: bool,
    always_handle: bool,
    on_edit: EditCallback,
    on_keydown: Option<KeydownCallback>,
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
            always_handle: true,
            on_edit: Rc::new(on_edit),
            on_keydown: None,
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

    pub fn on_keydown(
        mut self,
        callback: impl Fn(&mut EventContext, &Keystroke) -> DispatchEventResult + 'static,
    ) -> Self {
        self.on_keydown = Some(Rc::new(callback));
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
                (self.on_edit)(ctx, TextFieldEditAction::TypedCharacters(chars.clone()));
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
            Some(Event::KeyDown { keystroke, .. }) => {
                if keystroke.ctrl || keystroke.meta {
                    if keystroke.key.eq_ignore_ascii_case("v") {
                        if let Some(text) = read_clipboard_text() {
                            (self.on_edit)(ctx, TextFieldEditAction::Paste(text));
                            return true;
                        }
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
                match keystroke.key.as_str() {
                    "backspace" => {
                        (self.on_edit)(ctx, TextFieldEditAction::Backspace);
                        return true;
                    }
                    "enter" | "return" | "escape" | "tab" => {
                        if let Some(cb) = &self.on_keydown {
                            return matches!(cb(ctx, keystroke), DispatchEventResult::StopPropagation);
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
    use super::{display_with_preedit, pop_char, TextFieldEditAction, TextFieldState};

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
        state.apply(&mut draft, &TextFieldEditAction::SetMarkedText {
            marked_text: "ni".into(),
            selected_range: 0..2,
        });
        assert_eq!(state.marked_text, "ni");
        state.apply(&mut draft, &TextFieldEditAction::TypedCharacters("你".into()));
        assert_eq!(draft, "你");
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
        assert_eq!(compose_input_height("", ""), 36.0);
        assert!(compose_input_height("a\nb\nc", "") > 36.0);
    }
}
