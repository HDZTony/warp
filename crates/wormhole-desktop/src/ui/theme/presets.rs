//! Built-in theme families (Classic / Ocean / Sepia / Matrix / HAL 9000).

use std::collections::HashMap;

use super::types::{Theme, ThemeBackdrop, ThemeFamily, ThemeVariant};

fn colors(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn fonts(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    colors(pairs)
}

fn theme(
    id: &str,
    name: &str,
    is_dark: bool,
    color_pairs: &[(&str, &str)],
    font_pairs: &[(&str, &str)],
) -> Theme {
    Theme {
        id: id.to_string(),
        name: name.to_string(),
        is_dark,
        built_in: true,
        based_on: None,
        colors: colors(color_pairs),
        fonts: fonts(font_pairs),
        backdrop: ThemeBackdrop::default(),
    }
}

/// Classic Dark — empty overrides; runtime fills from `theme_generated` baseline.
pub fn classic_dark() -> Theme {
    theme("dark", "Classic Dark", true, &[], &[])
}

/// Classic Light — from `desktop-current.html` `data-theme="light"`.
pub fn classic_light() -> Theme {
    theme(
        "light",
        "Classic Light",
        false,
        &[
            ("canvas", "#F7F8FA"),
            ("bg", "#E9EDF3"),
            ("panel", "#FFFFFF"),
            ("panel_elevated", "#FFFFFF"),
            ("border", "#C5CBD6"),
            ("border_bright", "#9EA8B8"),
            ("text", "#1D2633"),
            ("muted", "#5E6B7A"),
            ("placeholder", "#8A95A5"),
            ("accent", "#315D9B"),
            ("accent_cool", "#315D9B"),
            ("danger", "#C4473A"),
            ("success", "#2F8F55"),
            ("warn", "#B08920"),
        ],
        &[],
    )
}

pub fn ocean_light() -> Theme {
    theme(
        "ocean",
        "Ocean Light",
        false,
        &[
            ("canvas", "#E9F2FC"),
            ("bg", "#D6E6F8"),
            ("panel", "#FFFFFF"),
            ("panel_elevated", "#F0F6FC"),
            ("border", "#CBDEF2"),
            ("border_bright", "#9BB8D9"),
            ("text", "#0F2438"),
            ("muted", "#2D4660"),
            ("placeholder", "#5A7A9A"),
            ("accent", "#4A83DD"),
            ("accent_cool", "#4A83DD"),
            ("danger", "#E85D4C"),
            ("success", "#2F8F55"),
            ("warn", "#B08920"),
        ],
        &[],
    )
}

pub fn ocean_dark() -> Theme {
    theme(
        "ocean-dark",
        "Ocean Dark",
        true,
        &[
            ("canvas", "#070C18"),
            ("bg", "#0E1628"),
            ("panel", "#161E34"),
            ("panel_elevated", "#283454"),
            ("border", "#303E62"),
            ("border_bright", "#465882"),
            ("text", "#E0E8F4"),
            ("muted", "#98A8C8"),
            ("placeholder", "#8090AC"),
            ("accent", "#60A5FA"),
            ("accent_cool", "#3B82F6"),
            ("danger", "#E85D4C"),
            ("success", "#5BC47A"),
            ("warn", "#E5C76B"),
        ],
        &[],
    )
}

pub fn sepia_light() -> Theme {
    theme(
        "sepia",
        "Sepia Light",
        false,
        &[
            ("canvas", "#F4ECDE"),
            ("bg", "#EEE4D2"),
            ("panel", "#FAF4E9"),
            ("panel_elevated", "#E8DCC6"),
            ("border", "#DED1BA"),
            ("border_bright", "#CEBEA2"),
            ("text", "#3C3226"),
            ("muted", "#786852"),
            ("placeholder", "#A09078"),
            ("accent", "#B4783C"),
            ("accent_cool", "#9C642E"),
            ("danger", "#C4473A"),
            ("success", "#4A8A4A"),
            ("warn", "#A07830"),
        ],
        &[("body", "georgia"), ("heading", "georgia")],
    )
}

pub fn sepia_dark() -> Theme {
    theme(
        "sepia-dark",
        "Sepia Dark",
        true,
        &[
            ("canvas", "#1A1611"),
            ("bg", "#221C16"),
            ("panel", "#28221A"),
            ("panel_elevated", "#3A3125"),
            ("border", "#403629"),
            ("border_bright", "#5A4C3A"),
            ("text", "#ECE4D6"),
            ("muted", "#B0A088"),
            ("placeholder", "#96866E"),
            ("accent", "#C8965A"),
            ("accent_cool", "#B07E46"),
            ("danger", "#E85D4C"),
            ("success", "#5BC47A"),
            ("warn", "#E5C76B"),
        ],
        &[("body", "georgia"), ("heading", "georgia")],
    )
}

pub fn matrix_dark() -> Theme {
    theme(
        "matrix",
        "Matrix",
        true,
        &[
            ("canvas", "#020804"),
            ("bg", "#06120A"),
            ("panel", "#0A1A0E"),
            ("panel_elevated", "#0E2414"),
            ("border", "#164022"),
            ("border_bright", "#246836"),
            ("text", "#86FFA8"),
            ("muted", "#3A9E5C"),
            ("placeholder", "#348452"),
            ("accent", "#00E65A"),
            ("accent_cool", "#00C44C"),
            ("danger", "#E85D4C"),
            ("success", "#00E65A"),
            ("warn", "#E5C76B"),
        ],
        &[("body", "consolas"), ("heading", "consolas"), ("mono", "consolas")],
    )
}

pub fn matrix_light() -> Theme {
    theme(
        "matrix-light",
        "Matrix Light",
        false,
        &[
            ("canvas", "#EAF5ED"),
            ("bg", "#E0F0E5"),
            ("panel", "#F6FCF8"),
            ("panel_elevated", "#D6ECD6"),
            ("border", "#C6E2CE"),
            ("border_bright", "#AACEB4"),
            ("text", "#0A3C1C"),
            ("muted", "#3C7A50"),
            ("placeholder", "#6EA07C"),
            ("accent", "#00A03C"),
            ("accent_cool", "#008432"),
            ("danger", "#C4473A"),
            ("success", "#00A03C"),
            ("warn", "#A07830"),
        ],
        &[("body", "consolas"), ("heading", "consolas"), ("mono", "consolas")],
    )
}

pub fn hal_dark() -> Theme {
    theme(
        "hal9000",
        "HAL 9000",
        true,
        &[
            ("canvas", "#080404"),
            ("bg", "#100808"),
            ("panel", "#140C0C"),
            ("panel_elevated", "#261414"),
            ("border", "#3C1C1C"),
            ("border_bright", "#602828"),
            ("text", "#F0E0E0"),
            ("muted", "#AA8282"),
            ("placeholder", "#826060"),
            ("accent", "#D61E1E"),
            ("accent_cool", "#E62828"),
            ("danger", "#D61E1E"),
            ("success", "#5BC47A"),
            ("warn", "#E5C76B"),
        ],
        &[],
    )
}

pub fn hal_light() -> Theme {
    theme(
        "hal9000-light",
        "HAL 9000 Light",
        false,
        &[
            ("canvas", "#F5EEEE"),
            ("bg", "#F0E4E4"),
            ("panel", "#FCF7F7"),
            ("panel_elevated", "#ECD9D9"),
            ("border", "#E2C8C8"),
            ("border_bright", "#CEAEAE"),
            ("text", "#3C1818"),
            ("muted", "#8C5A5A"),
            ("placeholder", "#AA8282"),
            ("accent", "#D22828"),
            ("accent_cool", "#B41E1E"),
            ("danger", "#D22828"),
            ("success", "#2F8F55"),
            ("warn", "#A07830"),
        ],
        &[],
    )
}

pub const THEME_FAMILIES: &[ThemeFamily] = &[
    ThemeFamily {
        id: "classic",
        name: "Classic",
        default_variant: ThemeVariant::Dark,
        light_id: Some("light"),
        dark_id: Some("dark"),
    },
    ThemeFamily {
        id: "ocean",
        name: "Ocean",
        default_variant: ThemeVariant::Light,
        light_id: Some("ocean"),
        dark_id: Some("ocean-dark"),
    },
    ThemeFamily {
        id: "sepia",
        name: "Sepia",
        default_variant: ThemeVariant::Light,
        light_id: Some("sepia"),
        dark_id: Some("sepia-dark"),
    },
    ThemeFamily {
        id: "matrix",
        name: "Matrix",
        default_variant: ThemeVariant::Dark,
        light_id: Some("matrix-light"),
        dark_id: Some("matrix"),
    },
    ThemeFamily {
        id: "hal9000",
        name: "HAL 9000",
        default_variant: ThemeVariant::Dark,
        light_id: Some("hal9000-light"),
        dark_id: Some("hal9000"),
    },
];

pub fn find_preset(id: &str) -> Option<Theme> {
    match id {
        "light" => Some(classic_light()),
        "dark" => Some(classic_dark()),
        "ocean" => Some(ocean_light()),
        "ocean-dark" => Some(ocean_dark()),
        "sepia" => Some(sepia_light()),
        "sepia-dark" => Some(sepia_dark()),
        "matrix" => Some(matrix_dark()),
        "matrix-light" => Some(matrix_light()),
        "hal9000" => Some(hal_dark()),
        "hal9000-light" => Some(hal_light()),
        _ => None,
    }
}

pub fn find_family(id: &str) -> Option<&'static ThemeFamily> {
    THEME_FAMILIES.iter().find(|f| f.id == id)
}

pub fn family_for_theme_id(id: &str) -> Option<&'static ThemeFamily> {
    THEME_FAMILIES
        .iter()
        .find(|f| f.light_id == Some(id) || f.dark_id == Some(id))
}

pub fn resolve_family_variant(family: &ThemeFamily, want_dark: bool) -> Theme {
    let id = if want_dark {
        family.dark_id.or(family.light_id)
    } else {
        family.light_id.or(family.dark_id)
    }
    .expect("theme family must have a variant");
    find_preset(id).expect("preset id from family must resolve")
}
