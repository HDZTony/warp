//! Cluster device grid + topology overlay; card width follows container (HTML `minmax(200px, 1fr)`).

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;

use warpui::elements::{
    AfterLayoutContext, Align, AppContext, Border, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, Element, EventContext, EventDispatchMode, EventHandler,
    Flex, LayoutContext, MainAxisAlignment, MainAxisSize, PaintContext, ParentElement, Point, Radius, Stack,
    SizeConstraint,
};
use warpui::fonts::FamilyId;

use crate::ui::cluster_layout::{
    card_height, cards_row_card_width, BODY_MIN_HEIGHT, CARD_GAP, TOPO_PAD,
};

const TOPO_HINT_TOP_MARGIN: f32 = 24.0;
use crate::ui::devices_actions::DevicesAction;
use crate::ui::hud_effects::ClusterTopology;
use crate::ui::icons;
use crate::ui::panel_primitives::{status_line, StatusTone, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::ClusterNodeDto;

const SHARE_BTN_HEIGHT: f32 = 28.0;
const SHARE_BTN_PAD_X: f32 = 10.0;

pub struct ClusterTopologyPanel {
    nodes: Vec<ClusterNodeDto>,
    local_node_id: String,
    hub_index: usize,
    mono: FamilyId,
    child: Box<dyn Element>,
    built_width: f32,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ClusterTopologyPanel {
    pub fn new(
        nodes: Vec<ClusterNodeDto>,
        local_node_id: String,
        hub_index: usize,
        mono: FamilyId,
    ) -> Self {
        Self {
            nodes,
            local_node_id,
            hub_index,
            mono,
            child: Flex::column().finish(),
            built_width: 0.0,
            size: None,
            origin: None,
        }
    }

    pub fn element(
        nodes: Vec<ClusterNodeDto>,
        local_node_id: String,
        hub_index: usize,
        mono: FamilyId,
    ) -> Box<dyn Element> {
        Box::new(Self::new(nodes, local_node_id, hub_index, mono))
    }

    fn rebuild(&mut self, container_width: f32) {
        let node_count = self.nodes.len().max(1);
        let card_w = cards_row_card_width(container_width, node_count);
        if self.built_width > 0.0 && (self.built_width - card_w).abs() < 0.5 {
            return;
        }
        self.built_width = card_w;

        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_alignment(MainAxisAlignment::Start);
        let nodes = self.nodes.clone();
        for (idx, node) in nodes.iter().enumerate() {
            let is_local = node.node_id == self.local_node_id;
            row.add_child(
                Container::new(node_card(node, is_local, card_w, self.mono))
                    .with_horizontal_margin(if idx + 1 < node_count {
                        CARD_GAP
                    } else {
                        0.0
                    })
                    .finish(),
            );
        }

        let card_h = card_height(card_w);
        let mut stack = Stack::new().with_event_dispatch_mode(EventDispatchMode::Waterfall);
        stack.add_child(ClusterTopology::new(node_count, self.hub_index).live());
        if node_count <= 2 {
            stack.add_child(
                Align::new(
                    Container::new(
                        ui_text::cluster_status("节点连线 · E2E 加密", self.mono)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(card_h + TOPO_PAD + TOPO_HINT_TOP_MARGIN)
                    .finish(),
                )
                .top_center()
                .finish(),
            );
        }
        stack.add_child(
            Container::new(row.finish())
                .with_uniform_padding(TOPO_PAD)
                .finish(),
        );
        self.child = stack.finish();
    }
}

impl Element for ClusterTopologyPanel {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let width = if constraint.max.x().is_finite() {
            constraint.max.x()
        } else {
            960.0
        };
        self.rebuild(width);
        let size = self.child.layout(constraint, ctx, app);
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
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

fn node_display_label(node: &ClusterNodeDto, is_local: bool) -> String {
    if is_local {
        format!("{} · 本机", node.os)
    } else {
        format!("{} · {}", node.os, node.hostname)
    }
}

fn share_files_button(
    node_id: String,
    online: bool,
    is_local: bool,
    mono: FamilyId,
) -> Box<dyn Element> {
    // Local share folders are on disk; browsing does not require gossip online.
    let clickable = online || is_local;
    let (border, color, bg) = if clickable {
        (theme::border_bright(), theme::text(), theme::panel())
    } else {
        (
            dim_color(theme::border_bright(), 0.55),
            theme::placeholder(),
            theme::panel_elevated(),
        )
    };
    let inner = Container::new(
        ConstrainedBox::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(
                    ui_text::cluster_ctrl("共享文件", mono)
                        .with_color(color)
                        .finish(),
                )
                .finish(),
        )
        .with_height(SHARE_BTN_HEIGHT)
        .finish(),
    )
    .with_horizontal_padding(SHARE_BTN_PAD_X)
    .with_background(bg)
    .with_border(Border::all(1.0).with_border_fill(border))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
    .finish();

    if clickable {
        EventHandler::new(inner)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::OpenNode(node_id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish()
    } else {
        inner
    }
}

fn node_card(
    node: &ClusterNodeDto,
    is_local: bool,
    card_width: f32,
    mono: FamilyId,
) -> Box<dyn Element> {
    let node_id = node.node_id.clone();
    let os_label = node.os.clone();
    let display_label = node_display_label(node, is_local);
    let status_text = if node.online { "在线" } else { "离线" };
    let status_tone = if node.online {
        StatusTone::Success
    } else {
        StatusTone::Placeholder
    };

    let mut body = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    body.add_child(
        ConstrainedBox::new(
            Container::new(
                ui_text::device_name(display_label, mono)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_vertical_margin(0.0)
            .finish(),
        )
        .with_min_height(32.0)
        .finish(),
    );
    body.add_child(status_line(status_text, mono, status_tone));
    body.add_child(
        Align::new(
            Container::new(share_files_button(node_id, node.online, is_local, mono))
                .with_vertical_margin(8.0)
                .finish(),
        )
        .left()
        .finish(),
    );

    let body_block = Container::new(
        ConstrainedBox::new(body.finish())
            .with_min_height(BODY_MIN_HEIGHT)
            .finish(),
    )
    .with_uniform_padding(12.0)
    .with_padding_top(10.0)
    .finish();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(icons::device_thumb(
        &os_label,
        node.online,
        theme::accent_cool(),
        mono,
        card_width,
    ));
    col.add_child(body_block);

    let border_color = if is_local {
        theme::accent()
    } else {
        theme::border()
    };

    let card = Container::new(
        ConstrainedBox::new(col.finish())
            .with_min_width(card_width)
            .with_width(card_width)
            .with_min_height(card_height(card_width))
            .finish(),
    )
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(border_color))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
    .finish();

    let card_h = card_height(card_width);
    let mut visual = Stack::new();
    visual.add_child(card);
    if is_local {
        visual.add_child(selected_corner_brackets());
    }

    ConstrainedBox::new(visual.finish())
        .with_width(card_width)
        .with_height(card_h)
        .finish()
}

fn dim_color(color: ColorU, factor: f32) -> ColorU {
    let scale = |component: u8| ((component as f32) * factor).round() as u8;
    ColorU::new(
        scale(color.r),
        scale(color.g),
        scale(color.b),
        scale(color.a),
    )
}

fn selected_corner_brackets() -> Box<dyn Element> {
    let corner = |top_left: bool| {
        let border = Border::new(1.0)
            .with_sides(top_left, top_left, !top_left, !top_left)
            .with_border_fill(theme::accent_cool());
        Container::new(
            ConstrainedBox::new(Flex::column().finish())
                .with_width(8.0)
                .with_height(8.0)
                .finish(),
        )
        .with_border(border)
        .finish()
    };

    let mut overlay = Stack::new();
    overlay.add_child(
        Align::new(
            Container::new(corner(true))
                .with_margin_top(6.0)
                .with_margin_left(6.0)
                .finish(),
        )
        .top_left()
        .finish(),
    );
    overlay.add_child(
        Align::new(
            Container::new(corner(false))
                .with_margin_bottom(6.0)
                .with_margin_right(6.0)
                .finish(),
        )
        .bottom_right()
        .finish(),
    );
    overlay.finish()
}
