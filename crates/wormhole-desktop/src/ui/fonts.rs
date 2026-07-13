use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{SingletonEntity as _, ViewContext};

/// Shown in Settings for OPPO Sans license compliance.
pub const UI_FONT_ATTRIBUTION: &str = "界面字体：OPPO Sans（OPPO Sans Fonts License Agreement）";

/// Bundled UI font from [`assets/字体/OPPO Sans 4.0.ttf`](../../../../../../assets/字体/OPPO Sans 4.0.ttf).
/// License: `assets/字体/OPPO Sans 4.0 License Notice.txt`.
const BUNDLED_UI_FONT_FAMILY: &str = "OPPO Sans 4.0";
const BUNDLED_UI_FONT_TTF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../assets/字体/OPPO Sans 4.0.ttf"
));

#[cfg(windows)]
const UI_FONT_CANDIDATES: &[&str] = &["Microsoft YaHei UI", "Microsoft YaHei", "Segoe UI"];

#[cfg(target_os = "macos")]
const UI_FONT_CANDIDATES: &[&str] = &["PingFang SC", ".AppleSystemUIFont"];

#[cfg(all(unix, not(target_os = "macos")))]
const UI_FONT_CANDIDATES: &[&str] = &["Noto Sans CJK SC", "WenQuanYi Micro Hei", "DejaVu Sans"];

#[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
const UI_FONT_CANDIDATES: &[&str] = &[];

/// System color-emoji fonts for chat picker glyphs (OPPO Sans has no emoji coverage).
#[cfg(windows)]
const EMOJI_FONT_CANDIDATES: &[&str] = &["Segoe UI Emoji"];

#[cfg(target_os = "macos")]
const EMOJI_FONT_CANDIDATES: &[&str] = &["Apple Color Emoji"];

#[cfg(all(unix, not(target_os = "macos")))]
const EMOJI_FONT_CANDIDATES: &[&str] = &["Noto Color Emoji", "Noto Emoji"];

#[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
const EMOJI_FONT_CANDIDATES: &[&str] = &[];

fn load_first_system_font(cache: &mut FontCache, candidates: &[&str]) -> Option<FamilyId> {
    candidates
        .iter()
        .find_map(|name| cache.load_system_font(name).ok())
}

fn load_bundled_ui_font(cache: &mut FontCache) -> Option<FamilyId> {
    if let Some(id) = cache.family_id_for_name(BUNDLED_UI_FONT_FAMILY) {
        return Some(id);
    }
    match cache.load_family_from_bytes(BUNDLED_UI_FONT_FAMILY, vec![BUNDLED_UI_FONT_TTF.to_vec()]) {
        Ok(id) => Some(id),
        Err(err) => {
            tracing::warn!(%err, "failed to load bundled OPPO Sans UI font");
            None
        }
    }
}

fn load_emoji_font_into_cache(cache: &mut FontCache) -> Option<FamilyId> {
    for name in EMOJI_FONT_CANDIDATES {
        if let Some(id) = cache.family_id_for_name(name) {
            return Some(id);
        }
        match cache.load_system_font(name) {
            Ok(id) => return Some(id),
            Err(err) => {
                tracing::debug!(%err, font = %name, "emoji font candidate unavailable");
            }
        }
    }
    tracing::warn!(
        candidates = ?EMOJI_FONT_CANDIDATES,
        "no system emoji font loaded; chat emoji picker may show missing glyphs"
    );
    None
}

/// Loads bundled and fallback fonts before any view constructs text.
pub fn warm_up_font_cache(ctx: &mut warpui::AppContext) {
    FontCache::handle(ctx).update(ctx, |cache, _| {
        if load_bundled_ui_font(cache).is_none() {
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            let _ = load_first_system_font(cache, UI_FONT_CANDIDATES);
            #[cfg(all(unix, not(target_os = "macos")))]
            tracing::warn!("bundled OPPO Sans missing; Linux dev uses default font family");
        }
        let _ = load_emoji_font_into_cache(cache);
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
            load_bundled_ui_font(cache).or_else(|| {
                #[cfg(not(all(unix, not(target_os = "macos"))))]
                {
                    load_first_system_font(cache, UI_FONT_CANDIDATES)
                }
                #[cfg(all(unix, not(target_os = "macos")))]
                {
                    None
                }
            })
        })
        .unwrap_or(FamilyId(0))
}

/// System color-emoji font for chat emoji picker cells.
///
/// Falls back to the UI font when no emoji family is available (glyphs may tofu).
pub fn load_emoji_font<E>(ctx: &mut ViewContext<E>) -> FamilyId
where
    E: warpui::Entity + warpui::View,
{
    let ui = load_ui_font(ctx);
    FontCache::handle(ctx)
        .update(ctx, |cache, _| load_emoji_font_into_cache(cache))
        .unwrap_or(ui)
}

/// True when `text` contains emoji presentation characters that need the system emoji font.
///
/// Covers common emoji blocks, VS16 (U+FE0F), and ZWJ (U+200D) sequences used by the picker.
pub fn text_contains_emoji(text: &str) -> bool {
    text.chars().any(|ch| {
        let c = ch as u32;
        matches!(c, 0x200D | 0xFE0F)
            || (0x2600..=0x27BF).contains(&c)
            || (0x1F000..=0x1FAFF).contains(&c)
    })
}

/// Font family for chat message body / draft / sidebar preview.
///
/// Uses the system emoji family when `text` contains emoji; otherwise the UI font
/// (OPPO Sans) so pure CJK stays sharp.
pub fn chat_message_font(ui_font: FamilyId, emoji_font: FamilyId, text: &str) -> FamilyId {
    if text_contains_emoji(text) {
        emoji_font
    } else {
        ui_font
    }
}

pub fn load_mono_font<E>(ctx: &mut ViewContext<E>, fallback: FamilyId) -> FamilyId
where
    E: warpui::Entity + warpui::View,
{
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = ctx;
        return fallback;
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
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
            assert!(face.glyph_index(ch).is_some(), "missing glyph for {ch}");
        }
    }

    #[test]
    fn emoji_font_candidates_are_non_empty_on_desktop_targets() {
        #[cfg(any(
            windows,
            target_os = "macos",
            all(unix, not(target_os = "macos"))
        ))]
        {
            assert!(!EMOJI_FONT_CANDIDATES.is_empty());
            for name in EMOJI_FONT_CANDIDATES {
                assert!(!name.is_empty());
            }
        }
    }

    #[test]
    fn text_contains_emoji_detects_faces_and_zwj() {
        assert!(text_contains_emoji("🤡"));
        assert!(text_contains_emoji("你好🤡"));
        assert!(text_contains_emoji("😶‍🌫️"));
        assert!(!text_contains_emoji("你好"));
        assert!(!text_contains_emoji(""));
    }

    #[test]
    fn chat_message_font_picks_emoji_family_only_when_needed() {
        let ui = FamilyId(1);
        let emoji = FamilyId(2);
        assert_eq!(chat_message_font(ui, emoji, "你好"), ui);
        assert_eq!(chat_message_font(ui, emoji, "🤡"), emoji);
        assert_eq!(chat_message_font(ui, emoji, "hi 🤡"), emoji);
    }
}
