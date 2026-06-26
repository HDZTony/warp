use warpui::elements::{Border, Container, CornerRadius, Radius};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::theme;
use crate::ui_text;

/// Matches `desktop-current.html` `--panel-pad` / `--radius`.
pub const SECTION_PADDING: f32 = 14.0;
pub const SECTION_GAP: f32 = 8.0;
pub const HUD_RADIUS: f32 = 2.0;

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

pub fn section_card(inner: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(inner)
        .with_background(theme::bg())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .with_uniform_padding(SECTION_PADDING)
        .finish()
}

pub fn section_title(text: impl Into<std::borrow::Cow<'static, str>>, font: FamilyId) -> Box<dyn Element> {
    let label = text.into().to_uppercase();
    ui_text::hud_title(label, font)
        .with_color(theme::accent_cool())
        .finish()
}

pub fn section_hint(text: impl Into<std::borrow::Cow<'static, str>>, font: FamilyId) -> Box<dyn Element> {
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
    ui_text::body(text, font).with_color(color).finish()
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
