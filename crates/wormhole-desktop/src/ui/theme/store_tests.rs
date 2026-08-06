use super::*;
use crate::ui::theme::types::{BackdropKind, ThemeBackdrop, ThemeVariant};

fn selection_classic() -> ThemeSelection {
    ThemeSelection::new(Some("classic".into()), ThemeVariant::Dark, vec![])
}

#[test]
fn resolve_classic_dark_uses_brand_baseline() {
    let theme = resolve_theme(&selection_classic(), true);
    assert_eq!(theme.id, "dark");
    assert!(theme.is_dark);
    let palette = runtime::palette_from_theme(&theme);
    assert_eq!(palette.canvas.r, 8);
}

#[test]
fn resolve_family_light_variant() {
    let mut sel = selection_classic();
    sel.theme_variant = ThemeVariant::Light;
    let theme = resolve_theme(&sel, true);
    assert_eq!(theme.id, "light");
    assert!(!theme.is_dark);
}

#[test]
fn system_variant_follows_os() {
    let mut sel = selection_classic();
    sel.theme_variant = ThemeVariant::System;
    assert_eq!(resolve_theme(&sel, true).id, "dark");
    assert_eq!(resolve_theme(&sel, false).id, "light");
}

#[test]
fn editing_builtin_auto_forks() {
    let mut sel = selection_classic();
    set_color_token(&mut sel, true, "accent", "#FF0000").unwrap();
    assert!(sel.active_theme_id.starts_with("custom-"));
    assert_eq!(sel.custom_themes.len(), 1);
    assert!(!sel.custom_themes[0].built_in);
    assert_eq!(sel.custom_themes[0].based_on.as_deref(), Some("dark"));
    assert_eq!(
        sel.custom_themes[0].colors.get("accent").map(String::as_str),
        Some("#FF0000")
    );
}

#[test]
fn reset_restores_based_on_preset() {
    let mut sel = selection_classic();
    set_color_token(&mut sel, true, "accent", "#FF0000").unwrap();
    reset_active_theme(&mut sel, true).unwrap();
    let theme = resolve_theme(&sel, true);
    let palette = runtime::palette_from_theme(&theme);
    // Brand accent warm cream, not red.
    assert_ne!(palette.accent.r, 255);
    assert_eq!(palette.accent.r, 253);
}

#[test]
fn json_roundtrip_custom_theme() {
    let mut sel = selection_classic();
    set_backdrop(
        &mut sel,
        true,
        ThemeBackdrop {
            kind: BackdropKind::Solid,
            image_url: None,
            dots: false,
        },
    );
    let theme = resolve_theme(&sel, true);
    let json = export_theme_json(&theme).unwrap();
    let imported = import_theme_json(&json).unwrap();
    assert!(!imported.built_in);
    assert_eq!(imported.backdrop.kind, BackdropKind::Solid);
}

#[test]
fn import_rejects_invalid_json() {
    assert!(import_theme_json("{not json").is_err());
}

#[test]
fn delete_custom_falls_back_to_classic() {
    let mut sel = selection_classic();
    set_color_token(&mut sel, true, "text", "#ABCDEF").unwrap();
    let id = sel.active_theme_id.clone();
    delete_custom_theme(&mut sel, &id).unwrap();
    assert_eq!(sel.active_theme_id, "classic");
    assert!(sel.custom_themes.is_empty());
}
