//! Rotating cluster refresh icon (`.cluster-refresh-btn.is-refreshing` in desktop-current.html).

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use resvg::usvg::{self, Tree};
use warpui::elements::{
    AfterLayoutContext, AppContext, EventContext, LayoutContext, PaintContext, Point,
    SizeConstraint,
};
use warpui::elements::{ConstrainedBox, LiveElement};
use warpui::Element;
use warpui_core::image_cache::StaticImage;

const CLUSTER_REFRESH_SVG: &[u8] = include_bytes!("../../assets/svg/cluster-refresh.svg");
const SPIN_PERIOD: Duration = Duration::from_millis(700);
const REPAINT_INTERVAL: Duration = Duration::from_millis(49);

fn cluster_refresh_tree() -> &'static Tree {
    static TREE: OnceLock<Tree> = OnceLock::new();
    TREE.get_or_init(|| {
        usvg::Tree::from_data(CLUSTER_REFRESH_SVG, &usvg::Options::default())
            .expect("cluster-refresh.svg must parse")
    })
}

fn render_rotated_frame(size_px: u32, angle_deg: f32) -> Arc<StaticImage> {
    let tree = cluster_refresh_tree();
    let mut pixmap = tiny_skia::Pixmap::new(size_px, size_px).expect("cluster refresh spin pixmap");
    let svg_size = tree.size();
    let sw = svg_size.width();
    let sh = svg_size.height();
    let scale = (size_px as f32 / sw).min(size_px as f32 / sh);
    let center = size_px as f32 / 2.0;
    let transform = tiny_skia::Transform::from_translate(center, center)
        .pre_rotate(angle_deg)
        .pre_scale(scale, scale)
        .pre_translate(-sw / 2.0, -sh / 2.0);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    let img = image::RgbaImage::from_vec(pixmap.width(), pixmap.height(), pixmap.take())
        .expect("cluster refresh pixmap size");
    StaticImage::from_rgba(img)
}

struct ClusterRefreshSpinIcon {
    started: Instant,
    color: ColorU,
    opacity: f32,
    icon_size: f32,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ClusterRefreshSpinIcon {
    fn live(color: ColorU, opacity: f32, icon_size: f32) -> Box<dyn Element> {
        Box::new(LiveElement::new(
            Box::new(Self {
                started: Instant::now(),
                color,
                opacity,
                icon_size,
                size: None,
                origin: None,
            }),
            REPAINT_INTERVAL,
        ))
    }
}

impl Element for ClusterRefreshSpinIcon {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _: &mut LayoutContext,
        _: &AppContext,
    ) -> Vector2F {
        let size = constraint.max;
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        let layout_size = self.size.unwrap_or_else(|| Vector2F::zero());
        if layout_size.x() <= 0.0 || layout_size.y() <= 0.0 {
            return;
        }

        let scale = ctx.scene.scale_factor();
        let px = ((self.icon_size * scale).round() as u32).max(1);
        let elapsed = self.started.elapsed();
        let progress =
            (elapsed.as_secs_f32() % SPIN_PERIOD.as_secs_f32()) / SPIN_PERIOD.as_secs_f32();
        let angle_deg = progress * 360.0;
        let frame = render_rotated_frame(px, angle_deg);
        let image_size = frame.size().to_f32() / scale;
        let draw_origin = origin + ((layout_size - image_size) / 2.0);
        self.origin = Some(Point::from_vec2f(draw_origin, ctx.scene.z_index()));
        ctx.scene.draw_icon(
            RectF::new(draw_origin, image_size),
            frame,
            self.opacity,
            self.color,
        );
        ctx.repaint_after(REPAINT_INTERVAL);
    }

    fn dispatch_event(
        &mut self,
        _: &warpui::event::DispatchedEvent,
        _: &mut EventContext,
        _: &AppContext,
    ) -> bool {
        false
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

pub fn cluster_refresh_spin_icon(color: ColorU, opacity: f32, icon_size: f32) -> Box<dyn Element> {
    ConstrainedBox::new(ClusterRefreshSpinIcon::live(color, opacity, icon_size))
        .with_width(icon_size)
        .with_height(icon_size)
        .finish()
}
