//! Cluster device grid + topology overlay; card width follows container (HTML `minmax(200px, 1fr)`).

use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;

use warpui::elements::{
    AfterLayoutContext, Align, AppContext, Border, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, Element, EventContext, EventDispatchMode,
    EventHandler, Flex, Hoverable, LayoutContext, MainAxisAlignment, MainAxisSize, MouseState,
    MouseStateHandle, PaintContext, ParentElement, Point, Radius, SizeConstraint, Stack,
};
use warpui::fonts::FamilyId;

use crate::ui::cluster_layout::{
    card_height, cards_row_card_width, BODY_MIN_HEIGHT, CARD_GAP, TOPO_PAD,
};

const TOPO_HINT_TOP_MARGIN: f32 = 24.0;
const DEVICE_CHROME_INSET: f32 = 6.0;
const DEVICE_DELETE_BTN: f32 = 26.0;
const SHARE_BTN_HEIGHT: f32 = 28.0;
const SHARE_BTN_PAD_X: f32 = 10.0;

use crate::ui::devices_actions::DevicesAction;
use crate::ui::hud_effects::ClusterTopology;
use crate::ui::icons;
use crate::ui::panel_primitives::{status_line, StatusTone, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::{
    ClusterNodeDto, NODE_PRESENCE_ONLINE, NODE_PRESENCE_SIGNED_IN,
};

pub struct ClusterTopologyPanel {
    nodes: Vec<ClusterNodeDto>,
    local_node_id: String,
    selected_node_id: String,
    hovered_node_id: String,
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
        selected_node_id: String,
        hovered_node_id: String,
        hub_index: usize,
        mono: FamilyId,
    ) -> Self {
        Self {
            nodes,
            local_node_id,
            selected_node_id,
            hovered_node_id,
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
        selected_node_id: String,
        hovered_node_id: String,
        hub_index: usize,
        mono: FamilyId,
    ) -> Box<dyn Element> {
        Box::new(Self::new(
            nodes,
            local_node_id,
            selected_node_id,
            hovered_node_id,
            hub_index,
            mono,
        ))
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
        let local_node_id = self.local_node_id.clone();
        let selected_node_id = self.selected_node_id.clone();
        let hovered_node_id = self.hovered_node_id.clone();
        for (idx, node) in nodes.iter().enumerate() {
            row.add_child(
                Container::new(node_card(
                    node,
                    &local_node_id,
                    &selected_node_id,
                    &hovered_node_id,
                    card_w,
                    self.mono,
                ))
                .with_horizontal_margin(if idx + 1 < node_count { CARD_GAP } else { 0.0 })
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

fn blend_color(base: ColorU, accent: ColorU, amount: f32) -> ColorU {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |from: u8, to: u8| -> u8 {
        (f32::from(from) * (1.0 - amount) + f32::from(to) * amount).round() as u8
    };
    ColorU::new(
        mix(base.r, accent.r),
        mix(base.g, accent.g),
        mix(base.b, accent.b),
        base.a,
    )
}

fn device_delete_button_border() -> ColorU {
    blend_color(theme::border(), theme::danger(), 0.45)
}

fn device_local_badge(mono: FamilyId) -> Box<dyn Element> {
    Container::new(
        ui_text::cluster_ctrl("本机".to_string(), mono)
            .with_color(theme::muted())
            .finish(),
    )
    .with_uniform_padding(2.0)
    .with_padding_left(7.0)
    .with_padding_right(7.0)
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(theme::border()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
    .finish()
}

fn device_delete_button(node_id: String, mono: FamilyId) -> Box<dyn Element> {
    let mouse_state: MouseStateHandle = Arc::new(Mutex::new(MouseState::default()));
    let button = Hoverable::new(mouse_state, move |state| {
        let (background, border_color) = if state.is_hovered() {
            (
                blend_color(theme::panel_elevated(), theme::danger(), 0.18),
                theme::danger(),
            )
        } else {
            (theme::panel_elevated(), device_delete_button_border())
        };
        Container::new(
            ConstrainedBox::new(
                Align::new(
                    ui_text::body("×".to_string(), mono)
                        .with_color(theme::danger())
                        .finish(),
                )
                .finish(),
            )
            .with_width(DEVICE_DELETE_BTN)
            .with_height(DEVICE_DELETE_BTN)
            .finish(),
        )
        .with_background(background)
        .with_border(Border::all(1.0).with_border_fill(border_color))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .finish()
    })
    .finish();

    EventHandler::new(button)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::OpenDeleteNodeModal(node_id.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn share_files_button(
    node_id: String,
    online: bool,
    is_local: bool,
    mono: FamilyId,
) -> Box<dyn Element> {
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

fn remove_device_button(
    device_id: Option<String>,
    node_id: String,
    mono: FamilyId,
) -> Box<dyn Element> {
    let inner = Container::new(
        ConstrainedBox::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(
                    ui_text::cluster_ctrl("移除设备", mono)
                        .with_color(theme::danger())
                        .finish(),
                )
                .finish(),
        )
        .with_height(SHARE_BTN_HEIGHT)
        .finish(),
    )
    .with_horizontal_padding(SHARE_BTN_PAD_X)
    .with_background(theme::panel())
    .with_border(Border::all(1.0).with_border_fill(theme::danger()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
    .finish();

    EventHandler::new(inner)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::RemoveClusterDevice {
                device_id: device_id.clone(),
                node_id: node_id.clone(),
            });
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn node_card(
    node: &ClusterNodeDto,
    local_node_id: &str,
    selected_node_id: &str,
    hovered_node_id: &str,
    card_width: f32,
    mono: FamilyId,
) -> Box<dyn Element> {
    let node_id = node.node_id.clone();
    let click_id = node_id.clone();
    let context_id = node_id.clone();
    let hover_in_id = node_id.clone();
    let os_label = node.os.clone();
    let is_local = node.node_id == local_node_id;
    let selected = node.node_id == selected_node_id;
    let hovered = node.node_id == hovered_node_id;
    let show_delete = !is_local && (hovered || selected);
    let display_label = node_display_label(node, is_local);
    let (status_text, status_tone) = node_presence_label(node);

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
            Container::new(share_files_button(
                node_id.clone(),
                node.online,
                is_local,
                mono,
            ))
            .with_vertical_margin(8.0)
            .finish(),
        )
        .left()
        .finish(),
    );
    if node.removable {
        body.add_child(
            Align::new(
                Container::new(remove_device_button(
                    node.device_id.clone(),
                    node.node_id.clone(),
                    mono,
                ))
                .with_vertical_margin(2.0)
                .finish(),
            )
            .left()
            .finish(),
        );
    }

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

    let border_color = if selected {
        theme::accent()
    } else if hovered {
        theme::accent_cool()
    } else if is_local {
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
    if selected || hovered {
        visual.add_child(selected_corner_brackets());
    }

    let interactive = EventHandler::new(visual.finish())
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::SetNodeHover(Some(hover_in_id.clone())));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::SetNodeHover(None));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(DevicesAction::NodeCardClick(click_id.clone()));
            DispatchEventResult::StopPropagation
        })
        .on_right_mouse_down(move |ctx, _, position| {
            ctx.dispatch_typed_action(DevicesAction::OpenDeviceContextMenu {
                node_id: context_id.clone(),
                x: position.x(),
                y: position.y(),
            });
            DispatchEventResult::StopPropagation
        })
        .finish();

    let mut card_stack = Stack::new();
    card_stack.add_child(interactive);
    if is_local {
        card_stack.add_child(
            Align::new(
                Container::new(device_local_badge(mono))
                    .with_margin_top(DEVICE_CHROME_INSET)
                    .with_margin_right(DEVICE_CHROME_INSET)
                    .finish(),
            )
            .top_right()
            .finish(),
        );
    } else if show_delete {
        card_stack.add_child(
            Align::new(
                Container::new(device_delete_button(node_id, mono))
                    .with_margin_top(DEVICE_CHROME_INSET)
                    .with_margin_right(DEVICE_CHROME_INSET)
                    .finish(),
            )
            .top_right()
            .finish(),
        );
    }

    ConstrainedBox::new(card_stack.finish())
        .with_width(card_width)
        .with_height(card_h)
        .finish()
}

fn node_presence_label(node: &ClusterNodeDto) -> (&'static str, StatusTone) {
    match node.presence_status.as_str() {
        NODE_PRESENCE_ONLINE => ("在线", StatusTone::Success),
        NODE_PRESENCE_SIGNED_IN => ("已登录 · 连接中", StatusTone::Warn),
        _ => ("离线", StatusTone::Placeholder),
    }
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
