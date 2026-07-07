use std::borrow::Cow;

use warpui::elements::Text;
use warpui::fonts::FamilyId;

/// Matches `desktop-current.html` `--body-size`.
pub const BODY_SIZE: f32 = 13.0;
/// Matches `desktop-current.html` `--mono-size`.
pub const MONO_SIZE: f32 = 12.0;
/// Matches `.ui-title` in `desktop-current.html`.
pub const HUD_TITLE_SIZE: f32 = 11.0;
/// Section headings (HTML `--title-size`).
pub const SECTION_TITLE_SIZE: f32 = 14.0;
/// Cluster toolbar + device card typography (HTML px + native legibility bump).
pub const CLUSTER_CTRL_SIZE: f32 = 12.0;
pub const CLUSTER_LABEL_SIZE: f32 = 13.0;
pub const CLUSTER_STATUS_SIZE: f32 = 13.0;
pub const DEVICE_NAME_SIZE: f32 = 12.0;
pub const DEVICE_META_SIZE: f32 = 13.0;
pub const DEVICE_OS_LABEL_SIZE: f32 = 12.0;
pub const CAPTION_GLYPH_SIZE: f32 = 22.0;
/// Matches `.tg-header-title` in `desktop-current.html`.
pub const CHAT_HEADER_TITLE_SIZE: f32 = 15.0;
/// Matches `.chat-compose-input` in `desktop-current.html`.
pub const CHAT_COMPOSE_FONT_SIZE: f32 = 14.0;

/// Matches `.tg-chat-name` in `desktop-current.html`.
pub const CHAT_SIDEBAR_NAME_SIZE: f32 = 14.0;
/// Matches `.tg-chat-time` in `desktop-current.html`.
pub const CHAT_SIDEBAR_TIME_SIZE: f32 = 11.0;
/// Matches `.tg-chat-preview` in `desktop-current.html`.
pub const CHAT_PREVIEW_SIZE: f32 = 13.0;
/// Matches `.tg-bubble` body in `desktop-current.html`.
pub const CHAT_BUBBLE_TEXT_SIZE: f32 = 14.0;
/// Matches `.tg-bubble-meta` in `desktop-current.html`.
pub const CHAT_BUBBLE_META_SIZE: f32 = 10.0;
/// Matches `.tg-header-status` in `desktop-current.html`.
pub const CHAT_HEADER_STATUS_SIZE: f32 = 12.0;
/// Matches `.tg-avatar` initials in `desktop-current.html`.
pub const CHAT_AVATAR_GLYPH_SIZE: f32 = 13.0;
/// Matches `.tg-avatar.sm` in `desktop-current.html`.
pub const CHAT_AVATAR_SM_GLYPH_SIZE: f32 = 12.0;
/// Matches profile `.tg-avatar` in `desktop-current.html`.
pub const CHAT_AVATAR_LG_GLYPH_SIZE: f32 = 18.0;

pub fn body(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, BODY_SIZE)
}

pub fn mono(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, MONO_SIZE)
}

pub fn hud_title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, HUD_TITLE_SIZE)
}

pub fn section_title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, SECTION_TITLE_SIZE)
}

pub fn cluster_ctrl(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CLUSTER_CTRL_SIZE)
}

pub fn cluster_label(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CLUSTER_LABEL_SIZE)
}

pub fn cluster_status(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CLUSTER_STATUS_SIZE)
}

pub fn device_name(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, DEVICE_NAME_SIZE)
}

pub fn device_meta(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, DEVICE_META_SIZE)
}

pub fn device_os_label(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, DEVICE_OS_LABEL_SIZE)
}

pub fn caption_glyph(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CAPTION_GLYPH_SIZE)
}

pub fn chat_header_title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_HEADER_TITLE_SIZE)
}

pub fn chat_compose(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_COMPOSE_FONT_SIZE)
}

pub fn chat_sidebar_name(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_SIDEBAR_NAME_SIZE)
}

pub fn chat_sidebar_time(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_SIDEBAR_TIME_SIZE)
}

pub fn chat_preview(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_PREVIEW_SIZE)
}

pub fn chat_bubble_text(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_BUBBLE_TEXT_SIZE)
}

pub fn chat_bubble_meta(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_BUBBLE_META_SIZE)
}

pub fn chat_header_status(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    Text::new(text, font, CHAT_HEADER_STATUS_SIZE)
}

pub fn chat_avatar_glyph(
    text: impl Into<Cow<'static, str>>,
    font: FamilyId,
    size: f32,
) -> Text {
    Text::new(text, font, size)
}

/// Legacy section heading; prefer [`section_title`] for HUD panels.
pub fn title(text: impl Into<Cow<'static, str>>, font: FamilyId) -> Text {
    section_title(text, font)
}
