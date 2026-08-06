use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Empty, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius,
    Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, EventContext};

use crate::ui::theme;
use crate::ui_text;

pub const TG_AVATAR_SIZE: f32 = 46.0;
pub const TG_AVATAR_SM_SIZE: f32 = 40.0;
pub const TG_AVATAR_LG_SIZE: f32 = 72.0;
pub const TG_BUBBLE_MAX_WIDTH: f32 = 520.0;
pub const TG_BUBBLE_RADIUS: f32 = 12.0;
pub const TG_BUBBLE_TAIL_RADIUS: f32 = 4.0;

/// Matches `desktop-current.html` `--panel-pad` / `--radius`.
pub const SECTION_PADDING: f32 = 14.0;
pub const SECTION_GAP: f32 = 8.0;
pub const HUD_RADIUS: f32 = 2.0;
/// `.agent-row-item` / `.agent-search-wrap` in `desktop-current.html`.
pub const AGENT_ROW_RADIUS: f32 = 10.0;
/// `.agent-sidebar-icon-btn` border-radius.
pub const AGENT_ICON_BTN_RADIUS: f32 = 7.0;
/// `.agent-composer` max content width.
pub const AGENT_THREAD_MAX_WIDTH: f32 = 720.0;
/// `.agent-thread` bottom padding — reserves space for overlay composer + folder bar (HTML: 220px).
pub const AGENT_THREAD_BOTTOM_PAD: f32 = 220.0;

/// Position a popover / context menu at viewport coordinates (matches `devices_view`).
pub fn positioned_context_menu(x: f32, y: f32, panel: Box<dyn Element>) -> Box<dyn Element> {
    let panel = EventHandler::new(panel)
        .with_automation_label("上下文菜单")
        .with_automation_id("shell:context_menu")
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

    Align::new(
        Container::new(panel)
            .with_margin_left(x.max(8.0))
            .with_margin_top(y.max(8.0))
            .finish(),
    )
    .top_left()
    .finish()
}

pub fn truncate_middle(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let head = max_chars / 2;
    let tail = max_chars - head - 1;
    let chars: Vec<char> = text.chars().collect();
    let start: String = chars.iter().take(head).collect();
    let end: String = chars.iter().skip(char_count - tail).collect();
    format!("{start}…{end}")
}

/// Matches `.view { height: 100% }` in `desktop-current.html` — Tab root fills HudBackdrop.
///
/// Uses `Stack` + [`Empty`] so the area always adopts the parent's finite max constraint
/// (Flex `MainAxisSize::Max` alone collapses when max height is unbounded).
pub fn tab_content_fill(inner: Box<dyn Element>) -> Box<dyn Element> {
    let mut stack = Stack::new();
    stack.add_child(Empty::new().finish());
    stack.add_child(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Expanded::new(1.0, inner).finish())
            .finish(),
    );
    stack.finish()
}

/// Mirrors `.agent-thread-inner` (`max-width: 720px; margin: 0 auto`).
///
/// Uses side [`Expanded`] spacers so inner flex rows receive a finite width (unlike bare
/// [`Align`] + [`ConstrainedBox::with_max_width`], which can pass an infinite main-axis max
/// during intrinsic measurement and trip flex `MainAxisSize::Max` asserts).
pub fn center_thread_content(inner: Box<dyn Element>) -> Box<dyn Element> {
    let content = ConstrainedBox::new(Expanded::new(1.0, inner).finish())
        .with_max_width(AGENT_THREAD_MAX_WIDTH)
        .finish();
    Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
        .with_child(content)
        .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
        .finish()
}

/// Horizontal center + max-width 720 for overlay composer (intrinsic height only).
///
/// Mirrors `.agent-composer-wrap` / `.agent-composer` in `desktop-current.html`.
/// Uses [`CrossAxisAlignment::Center`] — **not** `Stretch` (Stack full-height constraints
/// would vertically stretch the composer card). Do not use [`center_thread_content`] for composer.
pub fn center_composer_width(inner: Box<dyn Element>) -> Box<dyn Element> {
    let content = ConstrainedBox::new(inner)
        .with_max_width(AGENT_THREAD_MAX_WIDTH)
        .finish();
    Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
        .with_child(content)
        .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
        .finish()
}

/// Matches `.view-panel` — full-height panel background + `--panel-pad`.
pub fn view_panel(inner: Box<dyn Element>) -> Box<dyn Element> {
    tab_content_fill(
        Container::new(inner)
            .with_background(theme::panel())
            .with_uniform_padding(SECTION_PADDING)
            .with_border(Border::top(1.0).with_border_fill(theme::border()))
            .finish(),
    )
}

pub fn section_card(inner: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(inner)
        .with_background(theme::bg())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .with_uniform_padding(SECTION_PADDING)
        .finish()
}

pub fn section_title(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_title(text, font)
}

/// Matches `.ui-title` in `desktop-current.html` (mono HUD heading).
pub fn ui_title(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    let label = text.into().to_uppercase();
    ui_text::hud_title(label, font)
        .with_color(theme::accent_cool())
        .finish()
}

/// Matches `.agent-sidebar-head-label` — sentence case, muted.
pub fn agent_sidebar_label(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_text::body(text, font)
        .with_color(theme::muted())
        .finish()
}

/// `color-mix(in srgb, var(--panel) 38%, var(--canvas))`.
pub fn agent_sidebar_bg() -> pathfinder_color::ColorU {
    theme::mix_opaque(theme::panel(), theme::canvas(), 38)
}

/// `.agent-header` background: 55% panel + 45% canvas.
pub fn agent_header_bg() -> pathfinder_color::ColorU {
    theme::mix_opaque(theme::panel(), theme::canvas(), 55)
}

/// `.agent-row-item.active` background: 14% accent-cool + panel.
pub fn agent_row_active_bg() -> pathfinder_color::ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::panel(), 14)
}

/// `.tg-chat-item.active` — accent-cool 10% + panel.
pub fn chat_item_active_bg() -> ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::panel(), 10)
}

/// `.chat-sidebar-search-wrap` — canvas fill for contrast on `panel`.
pub fn chat_sidebar_search_bg() -> ColorU {
    theme::canvas()
}

/// Bordered search pill (`.chat-sidebar-search-wrap` / `.agent-search-wrap` / thread-search).
///
/// Parent must wrap the result in `Flex row Max` + `Expanded(1.0, …)` so the pill
/// stretches to the available width; optional `ConstrainedBox::with_width` inside `Expanded`
/// matches AI sidebar fixed width.
pub fn chat_search_pill(
    inner_row: Box<dyn Element>,
    background: ColorU,
    border: ColorU,
    padding_v: f32,
    padding_h: f32,
    radius: f32,
) -> Box<dyn Element> {
    Container::new(inner_row)
        .with_padding_left(padding_h)
        .with_padding_right(padding_h)
        .with_padding_top(padding_v)
        .with_padding_bottom(padding_v)
        .with_background(background)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(radius)))
        .finish()
}

/// `.agent-add-panel .add-icon` / chat attach popover icon circle.
pub const POPOVER_ICON_SIZE: f32 = 34.0;

/// Floating popover shell (AI composer access/model + chat attach/header/sticker).
pub fn popover_shell(width: f32, body: Box<dyn Element>) -> Box<dyn Element> {
    popover_shell_with_radius(width, AGENT_ROW_RADIUS, body)
}

/// Popover shell with an explicit corner radius (chat attach uses 12px).
pub fn popover_shell_with_radius(
    width: f32,
    radius: f32,
    body: Box<dyn Element>,
) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(body)
                .with_background(theme::panel_elevated())
                .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(radius)))
                .finish(),
        )
        .with_min_width(width)
        .with_width(width)
        .finish(),
    )
    .with_automation_label("弹出菜单")
    .with_automation_id("shell:popover")
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish()
}

/// Muted section title inside a popover (`.agent-access-panel-label`).
pub fn popover_menu_header(font: FamilyId, title: impl Into<String>) -> Box<dyn Element> {
    Container::new(agent_sidebar_label(title.into(), font))
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(12.0)
        .with_padding_bottom(8.0)
        .finish()
}

/// Icon circle + title + hint row (`.agent-add-panel` options).
pub fn popover_icon_option<F>(
    font: FamilyId,
    icon: Box<dyn Element>,
    title: impl Into<String>,
    hint: impl Into<String>,
    on_click: F,
) -> Box<dyn Element>
where
    F: 'static + FnMut(&mut EventContext, &AppContext, Vector2F) -> DispatchEventResult,
{
    let title = title.into();
    let hint = hint.into();
    let automation_label = title.clone();
    let text_col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(
            ui_text::body(title.clone(), font)
                .with_color(theme::text())
                .finish(),
        )
        .with_child(
            Container::new(section_hint(hint, font))
                .with_padding_top(2.0)
                .finish(),
        )
        .finish();
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(icon)
        .with_child(Container::new(text_col).with_margin_left(12.0).finish())
        .finish();
    Container::new(
        EventHandler::new(row)
            .with_automation_label(automation_label)
            .with_automation_id(format!("shell:popover:{title}"))
            .on_left_mouse_down(on_click)
            .finish(),
    )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .finish()
}

/// Icon circle + single-line label (chat attach: no header / no subtitle).
pub fn popover_icon_label_option<F>(
    font: FamilyId,
    icon: Box<dyn Element>,
    label: impl Into<String>,
    on_click: F,
) -> Box<dyn Element>
where
    F: 'static + FnMut(&mut EventContext, &AppContext, Vector2F) -> DispatchEventResult,
{
    let label = label.into();
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(icon)
        .with_child(
            Container::new(
                ui_text::body(label.clone(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_left(12.0)
            .finish(),
        )
        .finish();
    let automation_label = label.clone();
    Container::new(
        EventHandler::new(row)
            .with_automation_label(automation_label)
            .with_automation_id(format!("shell:popover:{label}"))
            .on_left_mouse_down(on_click)
            .finish(),
    )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .finish()
}

/// Single-line popover menu item (chat header / mute flyout).
pub fn popover_plain_item<F>(
    font: FamilyId,
    label: &str,
    danger: bool,
    has_flyout: bool,
    on_click: F,
) -> Box<dyn Element>
where
    F: 'static + FnMut(&mut EventContext, &AppContext, Vector2F) -> DispatchEventResult,
{
    let color = if danger {
        theme::danger()
    } else {
        theme::text()
    };
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max);
    row.add_child(
        ui_text::body(label.to_string(), font)
            .with_color(color)
            .finish(),
    );
    if has_flyout {
        row.add_child(
            Container::new(ui_text::body("›", font).with_color(theme::muted()).finish())
                .with_margin_left(8.0)
                .finish(),
        );
    }
    let item_label = label.to_string();
    EventHandler::new(
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .finish(),
    )
    .with_automation_label(item_label.clone())
    .with_automation_id(format!("shell:menu:{item_label}"))
    .on_left_mouse_down(on_click)
    .finish()
}

/// Horizontal rule inside a popover menu.
pub fn popover_menu_separator() -> Box<dyn Element> {
    ConstrainedBox::new(
        Container::new(Flex::row().finish())
            .with_vertical_margin(4.0)
            .with_horizontal_margin(8.0)
            .with_background(theme::border())
            .finish(),
    )
    .with_height(1.0)
    .finish()
}

/// `.tg-msg-row.in .tg-bubble` background.
pub fn chat_bubble_in_bg() -> ColorU {
    theme::panel_elevated()
}

/// `.tg-msg-row.out .tg-bubble` — accent-cool 18% + panel-elevated.
pub fn chat_bubble_out_bg() -> ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::panel_elevated(), 18)
}

/// Outgoing bubble border — accent-cool 35% + border.
pub fn chat_bubble_out_border() -> ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::border(), 35)
}

/// `.tg-avatar` background — accent-cool 14% + panel-elevated.
pub fn tg_avatar_bg() -> ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::panel_elevated(), 14)
}

/// `.tg-avatar` border — accent-cool 28% + border.
pub fn tg_avatar_border() -> ColorU {
    theme::mix_opaque(theme::accent_cool(), theme::border(), 28)
}

pub fn tg_avatar_glyph_size(diameter: f32) -> f32 {
    if diameter >= TG_AVATAR_LG_SIZE - 1.0 {
        ui_text::CHAT_AVATAR_LG_GLYPH_SIZE
    } else if diameter <= TG_AVATAR_SM_SIZE + 1.0 {
        ui_text::CHAT_AVATAR_SM_GLYPH_SIZE
    } else {
        ui_text::CHAT_AVATAR_GLYPH_SIZE
    }
}

/// Matches `.tg-avatar` / `.tg-avatar.sm` / profile avatar sizes.
///
/// Locks width/height with min+max so flex parents cannot stretch into an oval.
/// Do not wrap in `Shrinkable` — warpui `Shrinkable(factor)` allocates a fraction of
/// parent width (`0.0` → zero size → paint panic in sidebar scroll).
pub fn tg_avatar(initials: impl Into<String>, font: FamilyId, diameter: f32) -> Box<dyn Element> {
    let initials = initials.into();
    let glyph_size = tg_avatar_glyph_size(diameter);
    let inner = Container::new(
        Align::new(
            ui_text::chat_avatar_glyph(initials, font, glyph_size)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .finish(),
    )
    .with_background(tg_avatar_bg())
    .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
    .with_border(Border::all(1.0).with_border_fill(tg_avatar_border()))
    .finish();
    ConstrainedBox::new(inner)
        .with_width(diameter)
        .with_height(diameter)
        .with_min_width(diameter)
        .with_max_width(diameter)
        .with_min_height(diameter)
        .with_max_height(diameter)
        .finish()
}

/// `.conv-device-status .dot` — online indicator.
pub fn online_dot() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_width(5.0)
            .with_height(5.0)
            .finish(),
    )
    .with_background(theme::success())
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
    .finish()
}

pub fn section_hint(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
) -> Box<dyn Element> {
    ui_text::body(text, font)
        .with_color(theme::muted())
        .finish()
}

pub fn status_line(
    text: impl Into<std::borrow::Cow<'static, str>>,
    font: FamilyId,
    tone: StatusTone,
) -> Box<dyn Element> {
    let color = match tone {
        StatusTone::Neutral => theme::text(),
        StatusTone::Muted => theme::muted(),
        StatusTone::Placeholder => theme::placeholder(),
        StatusTone::Success => theme::success(),
        StatusTone::Warn => theme::warn(),
        StatusTone::Danger => theme::danger(),
    };
    ui_text::device_meta(text, font).with_color(color).finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Muted,
    Placeholder,
    Success,
    Warn,
    Danger,
}

#[cfg(test)]
mod tests {
    use super::truncate_middle;
    use super::{tg_avatar_glyph_size, TG_AVATAR_LG_SIZE, TG_AVATAR_SIZE, TG_AVATAR_SM_SIZE};
    use crate::ui_text;

    #[test]
    fn tg_avatar_glyph_size_matches_design_diameters() {
        assert_eq!(
            tg_avatar_glyph_size(TG_AVATAR_SIZE),
            ui_text::CHAT_AVATAR_GLYPH_SIZE
        );
        assert_eq!(
            tg_avatar_glyph_size(TG_AVATAR_SM_SIZE),
            ui_text::CHAT_AVATAR_SM_GLYPH_SIZE
        );
        assert_eq!(
            tg_avatar_glyph_size(TG_AVATAR_LG_SIZE),
            ui_text::CHAT_AVATAR_LG_GLYPH_SIZE
        );
    }

    #[test]
    fn tg_avatar_design_diameters_are_square() {
        assert_eq!(TG_AVATAR_SIZE, 46.0);
        assert_eq!(TG_AVATAR_SM_SIZE, 40.0);
        assert_eq!(TG_AVATAR_LG_SIZE, 72.0);
    }

    #[test]
    fn truncate_middle_short_unchanged() {
        assert_eq!(truncate_middle("abc", 10), "abc");
    }

    #[test]
    fn truncate_middle_long_path() {
        let path = "/very/long/path/to/some/deeply/nested/wormhole/data/directory";
        let out = truncate_middle(path, 24);
        assert!(out.contains('…'));
        assert!(out.chars().count() <= 24);
    }
}
