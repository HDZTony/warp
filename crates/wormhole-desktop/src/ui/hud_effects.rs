//! HUD backdrop and cluster topology overlays (`desktop-current.html`).

use std::f32::consts::PI;
use std::time::{Duration, Instant};

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{vec2f, Vector2F};

use warpui::elements::CornerRadius;
use warpui::elements::{
    AfterLayoutContext, AppContext, Element, EventContext, Fill, LayoutContext, LiveElement,
    PaintContext, Point, Radius, SizeConstraint,
};
use warpui::ClipBounds;

use crate::ui::cluster_layout::{
    card_height, cards_row_card_width, edge_points, node_anchors, CARD_GAP, CARD_MIN_WIDTH,
    TOPO_PAD,
};
use crate::ui::theme;

const GRID_SPACING: f32 = 24.0;
const REPAINT_INTERVAL: Duration = Duration::from_millis(32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkKind {
    Warm,
    Cool,
    Mesh,
}

struct LinkSpec {
    from: usize,
    to: usize,
    kind: LinkKind,
}

pub struct HudBackdrop {
    child: Box<dyn Element>,
    started: Instant,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl HudBackdrop {
    pub fn live(child: Box<dyn Element>) -> Box<dyn Element> {
        Box::new(LiveElement::new(
            Box::new(Self {
                child,
                started: Instant::now(),
                size: None,
                origin: None,
            }),
            REPAINT_INTERVAL,
        ))
    }
}

impl Element for HudBackdrop {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let mut size = self.child.layout(constraint, ctx, app);
        let max = constraint.max;
        if size.x().is_infinite() {
            size.set_x(if max.x().is_finite() { max.x() } else { 0.0 });
        }
        if size.y().is_infinite() {
            size.set_y(if max.y().is_finite() { max.y() } else { 0.0 });
        }
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let mut size = self.size.unwrap_or_else(|| vec2f(1.0, 1.0));
        if !size.x().is_finite() {
            size.set_x(1.0);
        }
        if size.y().is_infinite() {
            size.set_y(1.0);
        }
        let elapsed = self.started.elapsed().as_secs_f32();

        ctx.scene
            .start_layer(ClipBounds::BoundedBy(RectF::new(origin, size)));

        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(origin, size))
            .with_background(Fill::Solid(theme::canvas()));

        let grid_color = ColorU::new(222, 231, 247, 6);
        let mut x = 0.0f32;
        while x <= size.x() {
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + vec2f(x, 0.0),
                    vec2f(1.0, size.y()),
                ))
                .with_background(Fill::Solid(grid_color));
            x += GRID_SPACING;
        }
        let mut y = 0.0f32;
        while y <= size.y() {
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + vec2f(0.0, y),
                    vec2f(size.x(), 1.0),
                ))
                .with_background(Fill::Solid(grid_color));
            y += GRID_SPACING;
        }

        let drift = (elapsed * 4.0).sin() * GRID_SPACING * 0.15;
        let wash = ColorU::new(222, 231, 247, 8);
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                origin + vec2f(drift, 0.0),
                vec2f(size.x(), size.y() * 0.12),
            ))
            .with_background(Fill::Solid(wash));

        let scan_y = origin.y() + (elapsed / 8.0).fract() * size.y();
        let scan_h = size.y() * 0.28;
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                vec2f(origin.x(), scan_y),
                vec2f(size.x(), scan_h),
            ))
            .with_background(Fill::Solid(ColorU::new(222, 231, 247, 10)));

        ctx.scene.stop_layer();
        self.child.paint(origin, ctx, app);
    }

    fn dispatch_event(
        &mut self,
        event: &warpui::event::DispatchedEvent,
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

pub struct ClusterTopology {
    node_count: usize,
    hub_index: usize,
    started: Instant,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ClusterTopology {
    pub fn new(node_count: usize, hub_index: usize) -> Self {
        Self {
            node_count: node_count.max(1),
            hub_index: hub_index.min(node_count.saturating_sub(1)),
            started: Instant::now(),
            size: None,
            origin: None,
        }
    }

    fn links(&self) -> Vec<LinkSpec> {
        let n = self.node_count;
        let hub = self.hub_index;
        let mut links = Vec::new();
        if n >= 2 {
            for i in 0..n {
                if i == hub {
                    continue;
                }
                let kind = if i % 2 == 0 {
                    LinkKind::Warm
                } else {
                    LinkKind::Cool
                };
                links.push(LinkSpec {
                    from: hub,
                    to: i,
                    kind,
                });
            }
        }
        if n >= 3 {
            let others: Vec<usize> = (0..n).filter(|&i| i != hub).collect();
            if others.len() >= 2 {
                links.push(LinkSpec {
                    from: others[0],
                    to: others[1],
                    kind: LinkKind::Mesh,
                });
            }
        }
        links
    }

    fn link_color(kind: LinkKind) -> ColorU {
        match kind {
            LinkKind::Warm => theme::accent(),
            LinkKind::Cool => theme::accent_cool(),
            LinkKind::Mesh => theme::border_bright(),
        }
    }

    fn quad_point(p0: Vector2F, p1: Vector2F, p2: Vector2F, t: f32) -> Vector2F {
        let u = 1.0 - t;
        vec2f(
            p0.x() * u * u + p1.x() * 2.0 * u * t + p2.x() * t * t,
            p0.y() * u * u + p1.y() * 2.0 * u * t + p2.y() * t * t,
        )
    }

    fn midpoint(a: Vector2F, b: Vector2F) -> Vector2F {
        vec2f((a.x() + b.x()) * 0.5, (a.y() + b.y()) * 0.5)
    }

    fn draw_curve(
        &self,
        ctx: &mut PaintContext,
        origin: Vector2F,
        from: Vector2F,
        to: Vector2F,
        bow: f32,
        color: ColorU,
        phase: f32,
    ) {
        let mid = Self::midpoint(from, to);
        let delta = to - from;
        let len = delta.length().max(1.0);
        let normal = vec2f(-delta.y() / len, delta.x() / len);
        let control = mid + normal * bow;

        let segments = 28usize;
        for i in 0..segments {
            let t0 = i as f32 / segments as f32;
            let t1 = (i + 1) as f32 / segments as f32;
            let a = origin + Self::quad_point(from, control, to, t0);
            let b = origin + Self::quad_point(from, control, to, t1);
            let seg = b - a;
            let len_seg = seg.length().max(1.0);
            let center = Self::midpoint(a, b);
            let pulse = ((phase * PI * 2.0 + t0 * PI).sin() * 0.5 + 0.5) * 0.35 + 0.12;
            let alpha = (color.a as f32 * pulse) as u8;
            let mut c = color;
            c.a = alpha.max(18);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    vec2f(center.x() - len_seg * 0.5, center.y() - 0.75),
                    vec2f(len_seg, 1.5),
                ))
                .with_background(Fill::Solid(c))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(1.0)));
        }

        let packet_t = phase.fract();
        let packet = origin + Self::quad_point(from, control, to, packet_t);
        ctx.scene
            .draw_rect_without_hit_recording(RectF::new(
                vec2f(packet.x() - 3.0, packet.y() - 3.0),
                vec2f(6.0, 6.0),
            ))
            .with_background(Fill::Solid(theme::accent_cool()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(3.0)));
    }
}

impl ClusterTopology {
    pub fn live(self) -> Box<dyn Element> {
        Box::new(LiveElement::new(Box::new(self), REPAINT_INTERVAL))
    }
}

impl Element for ClusterTopology {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        let max = constraint.max;
        let size = vec2f(
            if max.x().is_finite() { max.x() } else { 0.0 },
            if max.y().is_finite() { max.y() } else { 0.0 },
        );
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let size = self.size.unwrap_or_else(|| vec2f(1.0, 1.0));
        let anchors = node_anchors(self.node_count, size);
        let card_w = cards_row_card_width(size.x(), self.node_count);
        let card_h = card_height(card_w);
        let elapsed = self.started.elapsed().as_secs_f32();

        ctx.scene
            .start_layer(ClipBounds::BoundedBy(RectF::new(origin, size)));
        ctx.scene.set_active_layer_click_through();

        if let Some(hub) = anchors.get(self.hub_index) {
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + *hub - vec2f(4.0, 4.0),
                    vec2f(8.0, 8.0),
                ))
                .with_background(Fill::Solid(theme::accent_cool()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)));
        }

        for (i, link) in self.links().into_iter().enumerate() {
            let Some(from) = anchors.get(link.from) else {
                continue;
            };
            let Some(to) = anchors.get(link.to) else {
                continue;
            };
            let (start, end) = edge_points(*from, *to, card_w, card_h);
            let bow = 28.0 + (i as f32 % 2.0) * 12.0;
            let phase = (elapsed / 1.4 + i as f32 * 0.2).fract();
            self.draw_curve(
                ctx,
                origin,
                start,
                end,
                bow,
                Self::link_color(link.kind),
                phase,
            );
        }

        ctx.scene.stop_layer();
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

const ENERGY_CYCLE: Duration = Duration::from_millis(2800);

const DEVICE_ENERGY_THUMB_HEIGHT: f32 = 132.0;

pub struct DeviceEnergyLines {
    started: Instant,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl DeviceEnergyLines {
    pub fn live() -> Box<dyn Element> {
        Box::new(LiveElement::new(
            Box::new(Self {
                started: Instant::now(),
                size: None,
                origin: None,
            }),
            REPAINT_INTERVAL,
        ))
    }

    fn line_phase(elapsed: Duration, delay_secs: f32, base_opacity: f32) -> (f32, f32) {
        let t = elapsed.as_secs_f32() + delay_secs;
        let progress = (t % ENERGY_CYCLE.as_secs_f32()) / ENERGY_CYCLE.as_secs_f32();
        let x = -1.1 + progress * 2.2;
        let fade = if progress < 0.15 {
            progress / 0.15
        } else if progress > 0.85 {
            (1.0 - progress) / 0.15
        } else {
            0.9
        };
        (x, base_opacity * fade)
    }

    fn paint_line(
        ctx: &mut PaintContext,
        origin: Vector2F,
        width: f32,
        height: f32,
        y_frac: f32,
        x_frac: f32,
        opacity: f32,
    ) {
        let line_w = width * 0.4;
        let center_x = origin.x() + width * 0.5 + x_frac * width * 0.5;
        let y = origin.y() + height * y_frac;
        let segments = 8usize;
        for i in 0..segments {
            let t = i as f32 / segments as f32;
            let edge = (t - 0.5).abs() * 2.0;
            let alpha = ((1.0 - edge).max(0.0) * opacity * 255.0) as u8;
            if alpha < 8 {
                continue;
            }
            let seg_w = line_w / segments as f32;
            let x = center_x - line_w * 0.5 + seg_w * i as f32;
            let mut color = theme::accent_cool();
            color.a = alpha;
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(vec2f(x, y), vec2f(seg_w + 0.5, 1.0)))
                .with_background(Fill::Solid(color));
        }
    }
}

impl Element for DeviceEnergyLines {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        let max = constraint.max;
        let size = vec2f(
            if max.x().is_finite() { max.x() } else { 0.0 },
            if max.y().is_finite() {
                max.y()
            } else {
                DEVICE_ENERGY_THUMB_HEIGHT
            },
        );
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let size = self
            .size
            .unwrap_or_else(|| vec2f(1.0, DEVICE_ENERGY_THUMB_HEIGHT));
        let elapsed = self.started.elapsed();
        let specs = [(0.22, 0.0, 1.0), (0.48, -1.2, 0.6), (0.74, -2.1, 0.4)];
        for (y_frac, delay, base_opacity) in specs {
            let (x_frac, opacity) = Self::line_phase(elapsed, delay, base_opacity);
            Self::paint_line(
                ctx,
                origin,
                size.x(),
                size.y(),
                y_frac,
                x_frac,
                opacity * 0.35,
            );
        }
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

#[cfg(test)]
mod tests {
    use super::ClusterTopology;

    #[test]
    fn topology_links_for_three_nodes() {
        let topo = ClusterTopology::new(3, 0);
        assert_eq!(topo.links().len(), 3);
    }
}
