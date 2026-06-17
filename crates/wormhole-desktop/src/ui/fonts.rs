use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{SingletonEntity as _, ViewContext};

pub fn load_ui_font<E>(ctx: &mut ViewContext<E>) -> FamilyId
where
    E: warpui::Entity + warpui::View,
{
    FontCache::handle(ctx)
        .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
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
