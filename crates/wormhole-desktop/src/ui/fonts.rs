use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{SingletonEntity as _, ViewContext};

/// Shown in Settings for OPPO Sans license compliance.
pub const UI_FONT_ATTRIBUTION: &str =
    "界面字体：OPPO Sans（OPPO Sans Fonts License Agreement）";

/// Bundled UI font from [`assets/字体/OPPO Sans 4.0.ttf`](../../../../../../assets/字体/OPPO Sans 4.0.ttf).
/// License: `assets/字体/OPPO Sans 4.0 License Notice.txt`.
const BUNDLED_UI_FONT_FAMILY: &str = "OPPO Sans 4.0";
const BUNDLED_UI_FONT_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../assets/字体/OPPO Sans 4.0.ttf"
));

#[cfg(windows)]
const UI_FONT_CANDIDATES: &[&str] = &[
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Segoe UI",
];

#[cfg(target_os = "macos")]
const UI_FONT_CANDIDATES: &[&str] = &["PingFang SC", ".AppleSystemUIFont"];

#[cfg(all(unix, not(target_os = "macos")))]
const UI_FONT_CANDIDATES: &[&str] = &[
    "Noto Sans CJK SC",
    "WenQuanYi Micro Hei",
    "DejaVu Sans",
];

#[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
const UI_FONT_CANDIDATES: &[&str] = &[];

fn load_first_system_font(cache: &mut FontCache, candidates: &[&str]) -> Option<FamilyId> {
    candidates
        .iter()
        .find_map(|name| cache.load_system_font(name).ok())
}

fn load_bundled_ui_font(cache: &mut FontCache) -> Option<FamilyId> {
    if let Some(id) = cache.family_id_for_name(BUNDLED_UI_FONT_FAMILY) {
        return Some(id);
    }
    match cache.load_family_from_bytes(
        BUNDLED_UI_FONT_FAMILY,
        vec![BUNDLED_UI_FONT_TTF.to_vec()],
    ) {
        Ok(id) => Some(id),
        Err(err) => {
            tracing::warn!(%err, "failed to load bundled OPPO Sans UI font");
            None
        }
    }
}

/// Loads bundled and fallback fonts before any view constructs text.
pub fn warm_up_font_cache(ctx: &mut warpui::AppContext) {
    FontCache::handle(ctx).update(ctx, |cache, _| {
        if load_bundled_ui_font(cache).is_none() {
            let _ = load_first_system_font(cache, UI_FONT_CANDIDATES);
        }
    });
}

/// Loads the primary UI font for shell labels and body text.
///
/// Uses bundled **OPPO Sans** from `assets/字体/` when present in the monorepo build;
/// falls back to platform system fonts with CJK coverage if loading fails.
pub fn load_ui_font<E>(ctx: &mut ViewContext<E>) -> FamilyId
where
    E: warpui::Entity + warpui::View,
{
    FontCache::handle(ctx)
        .update(ctx, |cache, _| {
            load_bundled_ui_font(cache).or_else(|| load_first_system_font(cache, UI_FONT_CANDIDATES))
        })
        .unwrap_or(FamilyId(0))
}

pub fn load_mono_font<E>(ctx: &mut ViewContext<E>, fallback: FamilyId) -> FamilyId
where
    E: warpui::Entity + warpui::View,
{
    FontCache::handle(ctx)
        .update(ctx, |cache, _| {
            cache
                .load_system_font("Consolas")
                .or_else(|_| cache.load_system_font("Cascadia Mono"))
                .ok()
        })
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_ui_font_includes_cjk_glyphs() {
        let face = owned_ttf_parser::Face::parse(BUNDLED_UI_FONT_TTF, 0)
            .expect("OPPO Sans TTF should parse");
        for ch in "盘设备聊天设置贴纸".chars() {
            assert!(
                face.glyph_index(ch).is_some(),
                "missing glyph for {ch}"
            );
        }
    }
}
