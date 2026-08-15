//! Chat thread background — light Telegram-style tint + sparse doodle tile,
//! or per-conversation wallpaper cover.

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};

use warpui::elements::{
    AfterLayoutContext, AppContext, Element, EventContext, Image, LayoutContext, PaintContext,
    Point, SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::ClipBounds;
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::panel_primitives::chat_thread_tint;
use crate::ui::theme;

/// Sparse original doodle tile (not tdesktop wallpaper).
const TILE: f32 = 64.0;

pub struct ChatThreadBackdrop {
    child: Box<dyn Element>,
    wallpaper: Option<Box<dyn Element>>,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ChatThreadBackdrop {
    pub fn new(child: Box<dyn Element>) -> Box<dyn Element> {
        Box::new(Self {
            child,
            wallpaper: None,
            size: None,
            origin: None,
        })
    }

    pub fn with_wallpaper(
        child: Box<dyn Element>,
        asset_id: impl Into<String>,
        loaded: bool,
    ) -> Box<dyn Element> {
        let wallpaper = if loaded {
            Some(
                Image::new(
                    AssetSource::Raw {
                        id: asset_id.into(),
                    },
                    CacheOption::BySize,
                )
                .finish(),
            )
        } else {
            None
        };
        Box::new(Self {
            child,
            wallpaper,
            size: None,
            origin: None,
        })
    }
}

fn paint_doodle_tile(origin: Vector2F, tile_origin: Vector2F, ctx: &mut PaintContext) {
    let ink = theme::with_alpha(theme::chat_date_bg(), 90);
    let soft = theme::with_alpha(theme::chat_date_bg(), 55);
    // Three dots + a small arc — original low-contrast pattern.
    let marks = [
        (10.0, 14.0, 2.4, ink),
        (42.0, 38.0, 2.0, soft),
        (54.0, 12.0, 1.8, soft),
    ];
    for (dx, dy, diam, color) in marks {
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                origin + tile_origin + vec2f(dx - diam * 0.5, dy - diam * 0.5),
                vec2f(diam, diam),
            ))
            .with_background(color);
    }
    // Tiny chevron / arc approximation (three small dots)
    for (dx, dy) in [(26.0, 50.0), (30.0, 47.0), (34.0, 50.0)] {
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                origin + tile_origin + vec2f(dx, dy),
                vec2f(1.4, 1.4),
            ))
            .with_background(soft);
    }
}

impl Element for ChatThreadBackdrop {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        if let Some(wallpaper) = self.wallpaper.as_mut() {
            wallpaper.layout(constraint, ctx, app);
        }
        let child_size = self.child.layout(constraint, ctx, app);
        let max = constraint.max;
        let size = vec2f(
            if max.x().is_finite() {
                max.x()
            } else {
                child_size.x()
            },
            if max.y().is_finite() {
                max.y()
            } else {
                child_size.y()
            },
        );
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        if let Some(wallpaper) = self.wallpaper.as_mut() {
            wallpaper.after_layout(ctx, app);
        }
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let size = self.size.unwrap_or_else(|| vec2f(1.0, 1.0));
        ctx.scene
            .start_layer(ClipBounds::BoundedBy(RectF::new(origin, size)));

        if let Some(wallpaper) = self.wallpaper.as_mut() {
            wallpaper.paint(origin, ctx, app);
            let overlay = theme::with_alpha(chat_thread_tint(), 168);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(origin, size))
                .with_background(overlay);
        } else {
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(origin, size))
                .with_background(chat_thread_tint());

            let mut y = 0.0;
            while y <= size.y() {
                let mut x = 0.0;
                while x <= size.x() {
                    paint_doodle_tile(origin, vec2f(x, y), ctx);
                    x += TILE;
                }
                y += TILE;
            }
        }

        ctx.scene.stop_layer();
        self.child.paint(origin, ctx, app);
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}
