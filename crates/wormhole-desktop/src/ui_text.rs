use std::borrow::Cow;

use warpui::elements::Text;
use warpui::fonts::FamilyId;

/// Matches `desktop-current.html` `--body-size`.
pub const BODY_SIZE: f32 = 13.0;
/// Matches `desktop-current.html` `--mono-size`.
pub const MONO_SIZE: f32 = 12.0;
/// Matches `.ui-title` in `desktop-current.html`.
pub const HUD_TITLE_SIZE: f32 = 11.0;
pub const CAPTION_GLYPH_SIZE: f32 = 22.0;

pub fn body(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, BODY_SIZE)
}

pub fn mono(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, MONO_SIZE)
}

pub fn hud_title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, HUD_TITLE_SIZE)
}

pub fn caption_glyph(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CAPTION_GLYPH_SIZE)
}

/// Legacy section heading; prefer [`hud_title`] for HUD panels.
pub fn title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    hud_title(text, font)
}
