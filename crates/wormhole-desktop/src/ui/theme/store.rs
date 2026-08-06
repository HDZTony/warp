//! Theme selection, auto-fork, CRUD, and JSON import/export.

use super::color::{color_u_to_hex, contrast_warning};
use super::presets::{
    family_for_theme_id, find_family, find_preset, resolve_family_variant, THEME_FAMILIES,
};
use super::runtime::{self, ActivePalette};
use super::tokens::ALL_COLOR_KEYS;
use super::types::{Theme, ThemeBackdrop, ThemeVariant};

/// Persisted theme selection state (also embedded in `DesktopUiPrefs`).
#[derive(Clone, Debug, Default)]
pub struct ThemeSelection {
    pub active_theme_id: String,
    pub theme_variant: ThemeVariant,
    pub custom_themes: Vec<Theme>,
}

impl ThemeSelection {
    pub fn new(
        active_theme_id: Option<String>,
        theme_variant: ThemeVariant,
        custom_themes: Vec<Theme>,
    ) -> Self {
        Self {
            active_theme_id: active_theme_id.unwrap_or_else(|| "classic".into()),
            theme_variant,
            custom_themes,
        }
    }
}

/// Whether OS reports dark appearance (`true`) or light (`false`).
pub fn resolve_want_dark(variant: ThemeVariant, system_is_dark: bool) -> bool {
    match variant {
        ThemeVariant::Light => false,
        ThemeVariant::Dark => true,
        ThemeVariant::System => system_is_dark,
    }
}

fn find_custom<'a>(customs: &'a [Theme], id: &str) -> Option<&'a Theme> {
    customs.iter().find(|t| t.id == id)
}

/// Resolve the concrete [`Theme`] for the current selection.
pub fn resolve_theme(selection: &ThemeSelection, system_is_dark: bool) -> Theme {
    let id = selection.active_theme_id.as_str();
    if let Some(custom) = find_custom(&selection.custom_themes, id) {
        return custom.clone();
    }
    // Family id → pick light/dark by variant.
    if let Some(family) = find_family(id) {
        let want_dark = resolve_want_dark(selection.theme_variant, system_is_dark);
        return resolve_family_variant(family, want_dark);
    }
    // Direct preset variant id (legacy / export).
    if let Some(preset) = find_preset(id) {
        return preset;
    }
    // Fallback: Classic + variant.
    let family = find_family("classic").expect("classic family exists");
    let want_dark = resolve_want_dark(selection.theme_variant, system_is_dark);
    resolve_family_variant(family, want_dark)
}

/// Apply selection to the process-wide palette.
pub fn apply_selection(selection: &ThemeSelection, system_is_dark: bool) -> Theme {
    let theme = resolve_theme(selection, system_is_dark);
    runtime::apply_theme(&theme);
    theme
}

/// Gallery family id for the current selection (custom themes report `based_on` family).
pub fn selected_family_id(selection: &ThemeSelection) -> String {
    let id = selection.active_theme_id.as_str();
    if let Some(custom) = find_custom(&selection.custom_themes, id) {
        if let Some(based) = custom.based_on.as_deref() {
            if let Some(family) = family_for_theme_id(based) {
                return family.id.to_string();
            }
        }
        return id.to_string();
    }
    if find_family(id).is_some() {
        return id.to_string();
    }
    if let Some(family) = family_for_theme_id(id) {
        return family.id.to_string();
    }
    "classic".into()
}

/// Ensure the active theme is editable; fork built-ins into a custom theme.
pub fn ensure_editable_custom(selection: &mut ThemeSelection, system_is_dark: bool) -> String {
    let current = resolve_theme(selection, system_is_dark);
    if !current.built_in {
        return current.id;
    }
    let source_id = current.id.clone();
    let custom_id = format!("custom-{source_id}");
    if find_custom(&selection.custom_themes, &custom_id).is_some() {
        selection.active_theme_id = custom_id.clone();
        return custom_id;
    }
    let mut forked = current;
    forked.id = custom_id.clone();
    forked.name = format!("{} (custom)", forked.name);
    forked.built_in = false;
    forked.based_on = Some(source_id);
    // Materialize full colour map so edits are explicit.
    let palette = runtime::palette_from_theme(&forked);
    forked.colors = palette_to_color_map(&palette);
    selection.custom_themes.push(forked);
    selection.active_theme_id = custom_id.clone();
    custom_id
}

fn palette_to_color_map(palette: &ActivePalette) -> std::collections::HashMap<String, String> {
    ALL_COLOR_KEYS
        .iter()
        .map(|key| ((*key).to_string(), color_u_to_hex(palette.color(key))))
        .collect()
}

fn mutate_active_custom(
    selection: &mut ThemeSelection,
    system_is_dark: bool,
    mutate: impl FnOnce(&mut Theme),
) {
    ensure_editable_custom(selection, system_is_dark);
    let id = selection.active_theme_id.clone();
    if let Some(theme) = selection.custom_themes.iter_mut().find(|t| t.id == id) {
        mutate(theme);
    }
}

pub fn set_color_token(
    selection: &mut ThemeSelection,
    system_is_dark: bool,
    key: &str,
    hex: &str,
) -> Result<(), String> {
    if !ALL_COLOR_KEYS.contains(&key) {
        return Err(format!("unknown colour token: {key}"));
    }
    if super::color::parse_hex(hex).is_none() {
        return Err(format!("invalid hex colour: {hex}"));
    }
    let normalized = if hex.trim().starts_with('#') {
        hex.trim().to_uppercase()
    } else {
        format!("#{}", hex.trim().to_uppercase())
    };
    mutate_active_custom(selection, system_is_dark, |theme| {
        theme.colors.insert(key.to_string(), normalized);
    });
    Ok(())
}

pub fn set_font_role(
    selection: &mut ThemeSelection,
    system_is_dark: bool,
    role: &str,
    choice_id: &str,
) -> Result<(), String> {
    if super::types::FontRole::parse(role).is_none() {
        return Err(format!("unknown font role: {role}"));
    }
    if super::tokens::font_choice(choice_id).is_none() {
        return Err(format!("unknown font choice: {choice_id}"));
    }
    mutate_active_custom(selection, system_is_dark, |theme| {
        theme.fonts.insert(role.to_string(), choice_id.to_string());
    });
    Ok(())
}

pub fn set_backdrop(
    selection: &mut ThemeSelection,
    system_is_dark: bool,
    backdrop: ThemeBackdrop,
) {
    mutate_active_custom(selection, system_is_dark, |theme| {
        theme.backdrop = backdrop;
    });
}

pub fn select_family(selection: &mut ThemeSelection, family_id: &str) -> Result<(), String> {
    if find_family(family_id).is_none()
        && find_custom(&selection.custom_themes, family_id).is_none()
    {
        return Err(format!("unknown theme: {family_id}"));
    }
    selection.active_theme_id = family_id.to_string();
    Ok(())
}

pub fn set_variant(selection: &mut ThemeSelection, variant: ThemeVariant) {
    selection.theme_variant = variant;
}

pub fn upsert_custom_theme(selection: &mut ThemeSelection, mut theme: Theme) {
    theme.built_in = false;
    if let Some(existing) = selection
        .custom_themes
        .iter_mut()
        .find(|t| t.id == theme.id)
    {
        *existing = theme;
    } else {
        selection.custom_themes.push(theme);
    }
}

pub fn delete_custom_theme(selection: &mut ThemeSelection, id: &str) -> Result<(), String> {
    let before = selection.custom_themes.len();
    selection.custom_themes.retain(|t| t.id != id);
    if selection.custom_themes.len() == before {
        return Err(format!("custom theme not found: {id}"));
    }
    if selection.active_theme_id == id {
        selection.active_theme_id = "classic".into();
    }
    Ok(())
}

/// Reset active custom theme colours/fonts/backdrop to its `based_on` preset.
pub fn reset_active_theme(selection: &mut ThemeSelection, system_is_dark: bool) -> Result<(), String> {
    let id = selection.active_theme_id.clone();
    let Some(custom) = find_custom(&selection.custom_themes, &id).cloned() else {
        return Err("only custom themes can be reset".into());
    };
    let base_id = custom
        .based_on
        .as_deref()
        .ok_or_else(|| "custom theme has no based_on preset".to_string())?;
    let base = find_preset(base_id).ok_or_else(|| format!("missing base preset: {base_id}"))?;
    let mut restored = base;
    restored.id = custom.id;
    restored.name = custom.name;
    restored.built_in = false;
    restored.based_on = custom.based_on;
    // Materialize colours from palette so Classic Dark empty map still resets visibly.
    let palette = runtime::palette_from_theme(&restored);
    restored.colors = palette_to_color_map(&palette);
    upsert_custom_theme(selection, restored);
    let _ = system_is_dark;
    Ok(())
}

pub fn export_theme_json(theme: &Theme) -> Result<String, String> {
    serde_json::to_string_pretty(theme).map_err(|e| e.to_string())
}

pub fn import_theme_json(raw: &str) -> Result<Theme, String> {
    let mut theme: Theme = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if theme.id.trim().is_empty() {
        theme.id = format!(
            "custom-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
    }
    if !theme.id.starts_with("custom-") {
        theme.id = format!("custom-{}", theme.id);
    }
    theme.built_in = false;
    if theme.name.trim().is_empty() {
        theme.name = "Imported theme".into();
    }
    Ok(theme)
}

pub fn active_contrast_warning(palette: &ActivePalette) -> bool {
    contrast_warning(palette.text, palette.canvas)
}

pub fn family_gallery() -> &'static [super::types::ThemeFamily] {
    THEME_FAMILIES
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
