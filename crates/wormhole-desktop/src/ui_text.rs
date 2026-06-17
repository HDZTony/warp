use std::borrow::Cow;

use warpui::elements::Text;
use warpui::fonts::FamilyId;

pub const BODY_SIZE: f32 = 14.0;
pub const MONO_SIZE: f32 = 13.0;
pub const TITLE_SIZE: f32 = 16.0;

pub fn body(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, BODY_SIZE)
}

pub fn mono(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, MONO_SIZE)
}

pub fn title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, TITLE_SIZE)
}
