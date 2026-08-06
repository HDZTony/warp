//! Runtime theme system for the Wormhole desktop shell.
//!
//! Classic Dark baseline colours still come from `theme_generated.rs`
//! (`uv run scripts/sync-brand-tokens.py`). All other families and custom
//! themes are partial overrides applied through [`runtime::ActivePalette`].

#[path = "../theme_generated.rs"]
#[allow(dead_code)]
mod generated;

pub mod color;
pub mod presets;
pub mod runtime;
pub mod store;
pub mod tokens;
pub mod types;

pub use color::{color_u_to_hex, mix_opaque, parse_hex, with_alpha};
pub use presets::THEME_FAMILIES;
pub use runtime::palette;
pub use store::{
    active_contrast_warning, apply_selection, delete_custom_theme, export_theme_json,
    family_gallery, import_theme_json, reset_active_theme, resolve_theme, select_family,
    selected_family_id, set_backdrop, set_color_token, set_font_role, set_variant,
    upsert_custom_theme, ThemeSelection,
};
pub use tokens::{COLOR_GROUPS, FONT_CHOICES, FontChoice};
pub use types::{BackdropKind, FontRole, Theme, ThemeVariant};

use pathfinder_color::ColorU;

/// Public colour accessors — same signatures as the old generated module.
pub fn canvas() -> ColorU {
    runtime::palette().canvas
}

pub fn bg() -> ColorU {
    runtime::palette().bg
}

pub fn accent() -> ColorU {
    runtime::palette().accent
}

pub fn accent_cool() -> ColorU {
    runtime::palette().accent_cool
}

pub fn panel() -> ColorU {
    runtime::palette().panel
}

pub fn panel_elevated() -> ColorU {
    runtime::palette().panel_elevated
}

pub fn border() -> ColorU {
    runtime::palette().border
}

pub fn border_bright() -> ColorU {
    runtime::palette().border_bright
}

pub fn text() -> ColorU {
    runtime::palette().text
}

pub fn muted() -> ColorU {
    runtime::palette().muted
}

pub fn placeholder() -> ColorU {
    runtime::palette().placeholder
}

pub fn danger() -> ColorU {
    runtime::palette().danger
}

pub fn success() -> ColorU {
    runtime::palette().success
}

pub fn warn() -> ColorU {
    runtime::palette().warn
}

pub fn accent_bg(alpha: u8) -> ColorU {
    let c = accent();
    ColorU::new(c.r, c.g, c.b, alpha)
}

pub fn accent_bg_default() -> ColorU {
    accent_bg(40)
}

pub fn accent_cool_bg(alpha: u8) -> ColorU {
    let c = accent_cool();
    ColorU::new(c.r, c.g, c.b, alpha)
}

pub fn accent_cool_bg_default() -> ColorU {
    accent_cool_bg(20)
}

pub fn is_dark() -> bool {
    runtime::palette().is_dark
}
