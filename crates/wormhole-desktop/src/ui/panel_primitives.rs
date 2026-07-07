use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Expanded,
    Flex, MainAxisSize, ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

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
/// `.agent-thread` bottom padding — reserves space for overlay composer (HTML: 120px).
pub const AGENT_THREAD_BOTTOM_PAD: f32 = 120.0;

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
    pathfinder_color::ColorU::new(21, 19, 27, 255)
}

/// `.agent-header` background: 55% panel + 45% canvas.
pub fn agent_header_bg() -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(27, 25, 35, 255)
}

/// `.agent-row-item.active` background: 14% accent-cool + panel.
pub fn agent_row_active_bg() -> pathfinder_color::ColorU {
    pathfinder_color::ColorU::new(67, 66, 81, 255)
}

/// `.tg-chat-item.active` — accent-cool 10% + panel.
pub fn chat_item_active_bg() -> ColorU {
    ColorU::new(60, 58, 73, 255)
}

/// `.chat-sidebar-search-wrap` — canvas 65% + panel.
pub fn chat_sidebar_search_bg() -> ColorU {
    ColorU::new(20, 18, 26, 255)
}

/// `.tg-msg-row.in .tg-bubble` background.
pub fn chat_bubble_in_bg() -> ColorU {
    theme::panel_elevated()
}

/// `.tg-msg-row.out .tg-bubble` — accent-cool 18% + panel-elevated.
pub fn chat_bubble_out_bg() -> ColorU {
    ColorU::new(92, 85, 94, 255)
}

/// Outgoing bubble border — accent-cool 35% + border.
pub fn chat_bubble_out_border() -> ColorU {
    ColorU::new(143, 141, 163, 255)
}

/// `.tg-avatar` background — accent-cool 14% + panel-elevated.
pub fn tg_avatar_bg() -> ColorU {
    ColorU::new(86, 78, 87, 255)
}

/// `.tg-avatar` border — accent-cool 28% + border.
pub fn tg_avatar_border() -> ColorU {
    ColorU::new(134, 132, 153, 255)
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
pub fn tg_avatar(initials: impl Into<String>, font: FamilyId, diameter: f32) -> Box<dyn Element> {
    let initials = initials.into();
    let glyph_size = tg_avatar_glyph_size(diameter);
    Container::new(
        ConstrainedBox::new(
            Align::new(
                ui_text::chat_avatar_glyph(initials, font, glyph_size)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .finish(),
        )
        .with_width(diameter)
        .with_height(diameter)
        .finish(),
    )
    .with_background(tg_avatar_bg())
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(diameter / 2.0)))
    .with_border(Border::all(1.0).with_border_fill(tg_avatar_border()))
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
