//! Process-wide active palette read by `theme::canvas()` etc.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use pathfinder_color::ColorU;

use super::color::parse_hex;
use super::tokens::{default_font_for_role, ALL_COLOR_KEYS};
use super::types::{BackdropKind, FontRole, Theme, ThemeBackdrop};

/// Resolved shell colours + backdrop + font choice ids.
#[derive(Clone, Debug)]
pub struct ActivePalette {
    pub canvas: ColorU,
    pub bg: ColorU,
    pub panel: ColorU,
    pub panel_elevated: ColorU,
    pub border: ColorU,
    pub border_bright: ColorU,
    pub text: ColorU,
    pub muted: ColorU,
    pub placeholder: ColorU,
    pub accent: ColorU,
    pub accent_cool: ColorU,
    pub danger: ColorU,
    pub success: ColorU,
    pub warn: ColorU,
    pub is_dark: bool,
    pub backdrop: ThemeBackdrop,
    pub fonts: HashMap<String, String>,
    /// Effective theme id currently applied (preset variant or custom).
    pub theme_id: String,
}

impl ActivePalette {
    pub fn color(&self, key: &str) -> ColorU {
        match key {
            "canvas" => self.canvas,
            "bg" => self.bg,
            "panel" => self.panel,
            "panel_elevated" => self.panel_elevated,
            "border" => self.border,
            "border_bright" => self.border_bright,
            "text" => self.text,
            "muted" => self.muted,
            "placeholder" => self.placeholder,
            "accent" => self.accent,
            "accent_cool" => self.accent_cool,
            "danger" => self.danger,
            "success" => self.success,
            "warn" => self.warn,
            _ => self.text,
        }
    }

    pub fn font_choice_id(&self, role: FontRole) -> String {
        self.fonts
            .get(role.as_str())
            .cloned()
            .unwrap_or_else(|| default_font_for_role(role).to_string())
    }
}

fn classic_dark_baseline() -> ActivePalette {
    use crate::ui::theme::generated;
    ActivePalette {
        canvas: generated::canvas(),
        bg: generated::bg(),
        panel: generated::panel(),
        panel_elevated: generated::panel_elevated(),
        border: generated::border(),
        border_bright: generated::border_bright(),
        text: generated::text(),
        muted: generated::muted(),
        placeholder: generated::placeholder(),
        accent: generated::accent(),
        accent_cool: generated::accent_cool(),
        danger: generated::danger(),
        success: generated::success(),
        warn: generated::warn(),
        is_dark: true,
        backdrop: ThemeBackdrop {
            kind: BackdropKind::Mesh,
            image_url: None,
            dots: true,
        },
        fonts: HashMap::new(),
        theme_id: "dark".into(),
    }
}

fn classic_light_baseline() -> ActivePalette {
    ActivePalette {
        canvas: ColorU::new(247, 248, 250, 255),
        bg: ColorU::new(233, 237, 243, 255),
        panel: ColorU::new(255, 255, 255, 255),
        panel_elevated: ColorU::new(255, 255, 255, 255),
        border: ColorU::new(197, 203, 214, 255),
        border_bright: ColorU::new(158, 168, 184, 255),
        text: ColorU::new(29, 38, 51, 255),
        muted: ColorU::new(94, 107, 122, 255),
        placeholder: ColorU::new(138, 149, 165, 255),
        accent: ColorU::new(49, 93, 155, 255),
        accent_cool: ColorU::new(49, 93, 155, 255),
        danger: ColorU::new(196, 71, 58, 255),
        success: ColorU::new(47, 143, 85, 255),
        warn: ColorU::new(176, 137, 32, 255),
        is_dark: false,
        backdrop: ThemeBackdrop {
            kind: BackdropKind::Solid,
            image_url: None,
            dots: false,
        },
        fonts: HashMap::new(),
        theme_id: "light".into(),
    }
}

fn set_key(palette: &mut ActivePalette, key: &str, color: ColorU) {
    match key {
        "canvas" => palette.canvas = color,
        "bg" => palette.bg = color,
        "panel" => palette.panel = color,
        "panel_elevated" => palette.panel_elevated = color,
        "border" => palette.border = color,
        "border_bright" => palette.border_bright = color,
        "text" => palette.text = color,
        "muted" => palette.muted = color,
        "placeholder" => palette.placeholder = color,
        "accent" => palette.accent = color,
        "accent_cool" => palette.accent_cool = color,
        "danger" => palette.danger = color,
        "success" => palette.success = color,
        "warn" => palette.warn = color,
        _ => {}
    }
}

/// Merge theme overrides onto the Classic light/dark baseline.
pub fn palette_from_theme(theme: &Theme) -> ActivePalette {
    let mut palette = if theme.is_dark {
        classic_dark_baseline()
    } else {
        classic_light_baseline()
    };
    palette.is_dark = theme.is_dark;
    palette.theme_id = theme.id.clone();
    palette.backdrop = theme.backdrop.clone();
    palette.fonts = theme.fonts.clone();
    for key in ALL_COLOR_KEYS {
        if let Some(hex) = theme.colors.get(*key) {
            if let Some(c) = parse_hex(hex) {
                set_key(&mut palette, key, c);
            }
        }
    }
    palette
}

static ACTIVE: OnceLock<Mutex<ActivePalette>> = OnceLock::new();

fn lock() -> &'static Mutex<ActivePalette> {
    ACTIVE.get_or_init(|| Mutex::new(classic_dark_baseline()))
}

pub fn palette() -> ActivePalette {
    lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub fn apply_theme(theme: &Theme) {
    let next = palette_from_theme(theme);
    *lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
}

pub fn apply_palette(next: ActivePalette) {
    *lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
}
