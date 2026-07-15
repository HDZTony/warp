//! Chat thread background — default deep-space dots or per-conversation wallpaper cover.

use pathfinder_color::ColorU;
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

use crate::ui::theme;

const DOT_SPACING: f32 = 48.0;

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
            let overlay = ColorU::new(18, 16, 24, 168);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(origin, size))
                .with_background(overlay);
        } else {
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(origin, size))
                .with_background(theme::canvas());

            let cool_glow = ColorU::new(222, 231, 247, 10);
            let warm_glow = ColorU::new(253, 249, 230, 8);
            let glow_w = size.x() * 0.55;
            let glow_h = size.y() * 0.55;
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + vec2f(size.x() * 0.08, size.y() * 0.18),
                    vec2f(glow_w, glow_h),
                ))
                .with_background(cool_glow);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + vec2f(size.x() * 0.52, size.y() * 0.48),
                    vec2f(glow_w * 0.85, glow_h * 0.75),
                ))
                .with_background(warm_glow);

            let dot = ColorU::new(100, 93, 117, 90);
            let mut y = 0.0;
            while y <= size.y() {
                let mut x = 0.0;
                while x <= size.x() {
                    ctx.scene
                        .draw_rect_without_hit_recording(RectF::new(
                            origin
                                + vec2f(x + DOT_SPACING * 0.5 - 0.6, y + DOT_SPACING * 0.5 - 0.6),
                            vec2f(1.2, 1.2),
                        ))
                        .with_background(dot);
                    x += DOT_SPACING;
                }
                y += DOT_SPACING;
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
