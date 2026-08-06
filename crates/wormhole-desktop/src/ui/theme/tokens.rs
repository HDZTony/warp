//! Editable shell token registry for Theme Studio.

use super::types::FontRole;

/// Surface (background) token keys.
pub const SURFACE_KEYS: &[&str] = &["canvas", "bg", "panel", "panel_elevated"];

/// Text token keys.
pub const TEXT_KEYS: &[&str] = &["text", "muted", "placeholder"];

/// Border token keys.
pub const BORDER_KEYS: &[&str] = &["border", "border_bright"];

/// Accent / status token keys (default studio accents group).
pub const ACCENT_KEYS: &[&str] = &["accent", "accent_cool", "danger", "success", "warn"];

/// Every colour token Theme Studio may edit.
pub const ALL_COLOR_KEYS: &[&str] = &[
    "canvas",
    "bg",
    "panel",
    "panel_elevated",
    "border",
    "border_bright",
    "text",
    "muted",
    "placeholder",
    "accent",
    "accent_cool",
    "danger",
    "success",
    "warn",
];

pub struct ColorGroup {
    pub id: &'static str,
    pub i18n_key: &'static str,
    pub keys: &'static [&'static str],
}

pub const COLOR_GROUPS: &[ColorGroup] = &[
    ColorGroup {
        id: "surfaces",
        i18n_key: "settings.theme.group.surfaces",
        keys: SURFACE_KEYS,
    },
    ColorGroup {
        id: "text",
        i18n_key: "settings.theme.group.text",
        keys: TEXT_KEYS,
    },
    ColorGroup {
        id: "borders",
        i18n_key: "settings.theme.group.borders",
        keys: BORDER_KEYS,
    },
    ColorGroup {
        id: "accents",
        i18n_key: "settings.theme.group.accents",
        keys: ACCENT_KEYS,
    },
];

/// Curated font choice offered per role in Theme Studio.
#[derive(Clone, Copy, Debug)]
pub struct FontChoice {
    pub id: &'static str,
    pub label: &'static str,
    /// System / bundled family name passed to FontCache.
    pub family_name: &'static str,
}

pub const FONT_CHOICES: &[FontChoice] = &[
    FontChoice {
        id: "oppo_sans",
        label: "OPPO Sans",
        family_name: "OPPO Sans 4.0",
    },
    FontChoice {
        id: "system",
        label: "System UI",
        family_name: "system",
    },
    FontChoice {
        id: "segoe",
        label: "Segoe UI",
        family_name: "Segoe UI",
    },
    FontChoice {
        id: "yahei",
        label: "Microsoft YaHei UI",
        family_name: "Microsoft YaHei UI",
    },
    FontChoice {
        id: "consolas",
        label: "Consolas",
        family_name: "Consolas",
    },
    FontChoice {
        id: "cascadia",
        label: "Cascadia Mono",
        family_name: "Cascadia Mono",
    },
    FontChoice {
        id: "georgia",
        label: "Georgia",
        family_name: "Georgia",
    },
];

pub fn font_choice(id: &str) -> Option<&'static FontChoice> {
    FONT_CHOICES.iter().find(|c| c.id == id)
}

pub fn default_font_for_role(role: FontRole) -> &'static str {
    match role {
        FontRole::Mono => "consolas",
        FontRole::Serif => "georgia",
        FontRole::Title | FontRole::Heading | FontRole::Body => "oppo_sans",
    }
}
