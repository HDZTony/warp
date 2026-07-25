//! Cluster device grid + topology overlay; card width follows container (HTML `minmax(200px, 1fr)`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::{vec2f, Vector2F};

use warpui::elements::{
    AfterLayoutContext, Align, AppContext, Border, ChildAnchor, ClippedScrollStateHandle,
    ClippedScrollable, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Element, EventContext, EventDispatchMode, EventHandler, Fill, Flex,
    Hoverable, LayoutContext, MainAxisAlignment, MainAxisSize, MouseState, MouseStateHandle,
    OffsetPositioning, PaintContext, ParentAnchor, ParentElement, ParentOffsetBounds, Point,
    Radius, ScrollbarWidth, SizeConstraint, Stack,
};
use warpui::fonts::FamilyId;

use crate::ui::cluster_layout::{
    card_height, grid_card_width, grid_column_count, BODY_PADDING_BOTTOM, BODY_PADDING_TOP,
    CARD_GAP, TOPO_PAD,
};

const TOPO_HINT_TOP_MARGIN: f32 = 24.0;
const DEVICE_CHROME_INSET: f32 = 6.0;
const DEVICE_DELETE_BTN: f32 = 26.0;
const DEVICE_ACTION_BTN_SIZE: f32 = 32.0;
const DEVICE_ACTION_GAP: f32 = 6.0;

use crate::ui::devices_actions::DevicesAction;
use crate::ui::hud_effects::ClusterTopology;
use crate::ui::icons;
use crate::ui::panel_primitives::{status_line, StatusTone, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::{
    ClusterNodeDto, NODE_PRESENCE_HANDSHAKE_FAILED, NODE_PRESENCE_HANDSHAKING,
    NODE_PRESENCE_ONLINE, NODE_PRESENCE_SIGNED_IN,
};
use wormhole_desktop_core::device_remarks::display_name_with_remark;

pub struct ClusterTopologyPanel {
    nodes: Vec<ClusterNodeDto>,
    local_node_id: String,
    selected_node_id: String,
    hovered_node_id: String,
    hub_index: usize,
    remarks: BTreeMap<String, String>,
    mono: FamilyId,
    scroll: ClippedScrollStateHandle,
    child: Box<dyn Element>,
    built_width: f32,
    built_columns: usize,
    built_node_count: usize,
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
        remarks: BTreeMap<String, String>,
        mono: FamilyId,
    ) -> Self {
        Self {
            nodes,
            local_node_id,
            selected_node_id,
            hovered_node_id,
            hub_index,
            remarks,
            mono,
            scroll: ClippedScrollStateHandle::new(),
            child: Flex::column().finish(),
            built_width: 0.0,
            built_columns: 0,
            built_node_count: 0,
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
        remarks: BTreeMap<String, String>,
        mono: FamilyId,
    ) -> Box<dyn Element> {
        Box::new(Self::new(
            nodes,
            local_node_id,
            selected_node_id,
            hovered_node_id,
            hub_index,
            remarks,
            mono,
        ))
    }

    fn rebuild(&mut self, container_width: f32) {
        let node_count = self.nodes.len().max(1);
        let columns = grid_column_count(container_width).max(1);
        let card_w = grid_card_width(container_width);
        if self.built_width > 0.0
            && (self.built_width - card_w).abs() < 0.5
            && self.built_columns == columns
            && self.built_node_count == node_count
        {
            return;
        }
        self.built_width = card_w;
        self.built_columns = columns;
        self.built_node_count = node_count;

        let nodes = self.nodes.clone();
        let local_node_id = self.local_node_id.clone();
        let selected_node_id = self.selected_node_id.clone();
        let hovered_node_id = self.hovered_node_id.clone();
        let remarks = self.remarks.clone();

        let mut grid = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min);

        let row_count = node_count.div_ceil(columns);
        for row_idx in 0..row_count {
            let start = row_idx * columns;
            let end = (start + columns).min(nodes.len().max(1));
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_main_axis_alignment(MainAxisAlignment::Start);
            if nodes.is_empty() {
                break;
            }
            for (col_idx, node) in nodes[start..end].iter().enumerate() {
                let is_last_in_row = col_idx + 1 >= end - start;
                row.add_child(
                    Container::new(node_card(
                        node,
                        &local_node_id,
                        &selected_node_id,
                        &hovered_node_id,
                        remarks.get(&node.node_id).map(String::as_str),
                        card_w,
                        self.mono,
                    ))
                    .with_horizontal_margin(if is_last_in_row { 0.0 } else { CARD_GAP })
                    .finish(),
                );
            }
            let is_last_row = row_idx + 1 >= row_count;
            grid.add_child(
                Container::new(row.finish())
                    .with_margin_bottom(if is_last_row { 0.0 } else { CARD_GAP })
                    .finish(),
            );
        }

        let card_h = card_height(card_w);
        let grid_content = Container::new(grid.finish())
            .with_uniform_padding(TOPO_PAD)
            .finish();

        let scroll = ClippedScrollable::vertical(
            self.scroll.clone(),
            grid_content,
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();

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
        stack.add_child(scroll);
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

fn node_display_label(node: &ClusterNodeDto, is_local: bool, remark: Option<&str>) -> String {
    display_name_with_remark(remark, || {
        if is_local {
            format!("{} · 本机", node.os)
        } else {
            format!("{} · {}", node.os, node.hostname)
        }
    })
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

/// Whether the terminal card「共享文件」button should accept clicks.
///
/// Live P2P browse needs `online` or `signed_in` + chat endpoint. Fully offline peers
/// remain browsable when the control-plane share roster is non-empty or this device
/// still has ClusterShare replicas (offline list falls back to local copies).
pub(crate) fn node_share_browsable(node: &ClusterNodeDto, is_local: bool) -> bool {
    if is_local || node.online {
        return true;
    }
    let has_endpoint = node
        .chat_endpoint_id
        .as_deref()
        .is_some_and(|id| !id.trim().is_empty());
    if node.presence_status == NODE_PRESENCE_SIGNED_IN && has_endpoint {
        return true;
    }
    !node.share_volumes.is_empty() || node.has_local_share_replicas
}

pub(crate) fn node_remote_desktop_available(node: &ClusterNodeDto, is_local: bool) -> bool {
    !is_local
        && node.online
        && node
            .chat_endpoint_id
            .as_deref()
            .is_some_and(|endpoint| !endpoint.trim().is_empty())
}

fn share_volume_count_hint(count: usize, mono: FamilyId) -> Box<dyn Element> {
    let label = if count == 0 {
        "尚未发布共享".to_string()
    } else {
        format!("{count} 个共享")
    };
    let tone = if count == 0 {
        theme::placeholder()
    } else {
        theme::muted()
    };
    Container::new(ui_text::cluster_ctrl(label, mono).with_color(tone).finish())
        .with_vertical_margin(2.0)
        .finish()
}

fn device_action_tooltip(label: &'static str, mono: FamilyId) -> Box<dyn Element> {
    Container::new(
        ui_text::cluster_ctrl(label, mono)
            .with_color(theme::text())
            .finish(),
    )
    .with_vertical_padding(5.0)
    .with_horizontal_padding(8.0)
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
    .finish()
}

fn device_action_button(
    kind: icons::DeviceActionIconKind,
    label: &'static str,
    enabled: bool,
    danger: bool,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mouse_state: MouseStateHandle = Arc::new(Mutex::new(MouseState::default()));
    Hoverable::new(mouse_state, move |state| {
        let pointer_hovered = state.is_hovered();
        let hovered = pointer_hovered && enabled;
        let (color, background) = if !enabled {
            (theme::muted(), None)
        } else if danger {
            (
                theme::danger(),
                if hovered {
                    Some(blend_color(theme::panel_elevated(), theme::danger(), 0.18))
                } else {
                    None
                },
            )
        } else {
            (
                theme::text(),
                if hovered {
                    Some(theme::accent_cool_bg(16))
                } else {
                    None
                },
            )
        };

        let button_content =
            ConstrainedBox::new(Align::new(icons::device_action_icon(kind, color)).finish())
                .with_width(DEVICE_ACTION_BTN_SIZE)
                .with_height(DEVICE_ACTION_BTN_SIZE)
                .finish();
        let button = match background {
            Some(background) => Container::new(button_content)
                .with_background(background)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .finish(),
            None => Container::new(button_content).finish(),
        };

        let mut stack = Stack::new();
        stack.add_child(button);
        if pointer_hovered {
            stack.add_positioned_overlay_child(
                device_action_tooltip(label, mono),
                OffsetPositioning::offset_from_parent(
                    vec2f(0.0, -6.0),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopMiddle,
                    ChildAnchor::BottomMiddle,
                ),
            );
        }
        stack.finish()
    })
    .finish()
}

fn share_files_button(node_id: String, clickable: bool, mono: FamilyId) -> Box<dyn Element> {
    let inner = device_action_button(
        icons::DeviceActionIconKind::ShareFiles,
        "共享文件",
        clickable,
        false,
        mono,
    );

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

fn remote_desktop_button(node_id: String, available: bool, mono: FamilyId) -> Box<dyn Element> {
    let inner = device_action_button(
        icons::DeviceActionIconKind::RemoteDesktop,
        "远程桌面",
        available,
        false,
        mono,
    );

    if available {
        EventHandler::new(inner)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(DevicesAction::OpenRemoteDesktop(node_id.clone()));
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
    let inner = device_action_button(
        icons::DeviceActionIconKind::RemoveDevice,
        "移除设备",
        true,
        true,
        mono,
    );

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
    remark: Option<&str>,
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
    let display_label = node_display_label(node, is_local, remark);
    let (status_text, status_tone) = node_presence_label(node);

    // Clickable region: thumb + name/status/share count (not action buttons).
    let mut info = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    info.add_child(
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
    info.add_child(status_line(status_text, mono, status_tone));
    info.add_child(share_volume_count_hint(node.share_volumes.len(), mono));

    let info_block = Container::new(info.finish())
        .with_horizontal_padding(12.0)
        .with_padding_top(BODY_PADDING_TOP)
        .finish();

    let online = node.online;
    // Clicks on thumb/info only — hover for the whole card is wrapped below so the
    // delete control stays inside the hover region.
    let clickable = EventHandler::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(icons::device_thumb(
                &os_label,
                online,
                theme::accent_cool(),
                mono,
                card_width,
            ))
            .with_child(info_block)
            .finish(),
    )
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

    // Action buttons sit outside the card click handler so a single click opens.
    let mut actions = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    actions.add_child(
        Container::new(share_files_button(
            node_id.clone(),
            node_share_browsable(node, is_local),
            mono,
        ))
        .with_margin_right(if !is_local || node.removable {
            DEVICE_ACTION_GAP
        } else {
            0.0
        })
        .finish(),
    );
    if !is_local {
        actions.add_child(
            Container::new(remote_desktop_button(
                node_id.clone(),
                node_remote_desktop_available(node, is_local),
                mono,
            ))
            .with_margin_right(if node.removable {
                DEVICE_ACTION_GAP
            } else {
                0.0
            })
            .finish(),
        );
    }
    if node.removable {
        actions.add_child(remove_device_button(
            node.device_id.clone(),
            node.node_id.clone(),
            mono,
        ));
    }
    let actions_block = Container::new(
        Align::new(
            Container::new(actions.finish())
                .with_margin_top(8.0)
                .finish(),
        )
        .left()
        .finish(),
    )
    .with_horizontal_padding(12.0)
    .with_padding_bottom(BODY_PADDING_BOTTOM)
    .finish();

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(clickable);
    col.add_child(actions_block);

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
    let mut card_stack = Stack::new();
    card_stack.add_child(card);
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

    ConstrainedBox::new(
        Hoverable::new(Arc::new(Mutex::new(MouseState::default())), move |_| {
            card_stack.finish()
        })
        .on_hover(move |hovered, ctx, _, _| {
            if hovered {
                ctx.dispatch_typed_action(DevicesAction::SetNodeHover(Some(hover_in_id.clone())));
            } else {
                ctx.dispatch_typed_action(DevicesAction::ClearNodeHoverIf(hover_in_id.clone()));
            }
        })
        .finish(),
    )
    .with_width(card_width)
    .with_height(card_h)
    .finish()
}

fn node_presence_label(node: &ClusterNodeDto) -> (&'static str, StatusTone) {
    match node.presence_status.as_str() {
        NODE_PRESENCE_ONLINE => ("在线", StatusTone::Success),
        NODE_PRESENCE_SIGNED_IN => ("已登录 · 连接中", StatusTone::Warn),
        NODE_PRESENCE_HANDSHAKING => ("握手中", StatusTone::Warn),
        NODE_PRESENCE_HANDSHAKE_FAILED => ("连接失败", StatusTone::Danger),
        _ => ("离线", StatusTone::Placeholder),
    }
}

#[cfg(test)]
mod node_share_browsable_tests {
    use super::{node_remote_desktop_available, node_share_browsable};
    use wormhole_desktop_core::cluster_commands::{
        ClusterNodeDto, ShareVolumeRosterDto, NODE_PRESENCE_OFFLINE, NODE_PRESENCE_ONLINE,
        NODE_PRESENCE_SIGNED_IN,
    };

    fn sample_node(
        online: bool,
        presence: &str,
        endpoint: Option<&str>,
        share_count: usize,
    ) -> ClusterNodeDto {
        ClusterNodeDto {
            node_id: "remote".into(),
            device_id: None,
            chat_endpoint_id: endpoint.map(str::to_string),
            chat_bootstrap_addrs: Vec::new(),
            hostname: "host".into(),
            os: "windows".into(),
            roles: Vec::new(),
            online,
            presence_status: presence.into(),
            cpu_cores: 1,
            memory_total: 1,
            storage_total: 0,
            storage_free: 0,
            billing_node_score: None,
            billing_expired: false,
            role: "member".into(),
            removable: false,
            revoked: false,
            server_member_confirmed: true,
            same_account: true,
            pending_handshake: false,
            handshake_error: None,
            share_volumes: (0..share_count)
                .map(|i| ShareVolumeRosterDto {
                    volume_id: format!("v{i}"),
                    name: format!("share-{i}"),
                })
                .collect(),
            has_local_share_replicas: false,
        }
    }

    #[test]
    fn local_always_browsable() {
        let node = sample_node(false, NODE_PRESENCE_OFFLINE, None, 0);
        assert!(node_share_browsable(&node, true));
    }

    #[test]
    fn online_remote_browsable() {
        let node = sample_node(true, NODE_PRESENCE_ONLINE, Some("abc"), 1);
        assert!(node_share_browsable(&node, false));
    }

    #[test]
    fn signed_in_with_endpoint_browsable() {
        let node = sample_node(false, NODE_PRESENCE_SIGNED_IN, Some("endpoint-hex"), 2);
        assert!(node_share_browsable(&node, false));
    }

    #[test]
    fn offline_with_share_roster_browsable() {
        let node = sample_node(false, NODE_PRESENCE_OFFLINE, None, 1);
        assert!(node_share_browsable(&node, false));
    }

    #[test]
    fn offline_with_local_replicas_browsable() {
        let mut node = sample_node(false, NODE_PRESENCE_OFFLINE, None, 0);
        node.has_local_share_replicas = true;
        assert!(node_share_browsable(&node, false));
    }

    #[test]
    fn offline_without_roster_or_replicas_not_browsable() {
        let node = sample_node(false, NODE_PRESENCE_OFFLINE, Some("endpoint-hex"), 0);
        assert!(!node_share_browsable(&node, false));
    }

    #[test]
    fn signed_in_without_endpoint_not_browsable_without_roster() {
        let node = sample_node(false, NODE_PRESENCE_SIGNED_IN, None, 0);
        assert!(!node_share_browsable(&node, false));
    }

    #[test]
    fn signed_in_blank_endpoint_not_browsable_without_roster() {
        let node = sample_node(false, NODE_PRESENCE_SIGNED_IN, Some("  "), 0);
        assert!(!node_share_browsable(&node, false));
    }

    #[test]
    fn remote_desktop_requires_remote_online_node_with_endpoint() {
        let ready = sample_node(true, NODE_PRESENCE_ONLINE, Some("endpoint-hex"), 0);
        assert!(node_remote_desktop_available(&ready, false));
        assert!(!node_remote_desktop_available(&ready, true));

        let offline = sample_node(false, NODE_PRESENCE_OFFLINE, Some("endpoint-hex"), 0);
        assert!(!node_remote_desktop_available(&offline, false));

        let missing = sample_node(true, NODE_PRESENCE_ONLINE, None, 0);
        assert!(!node_remote_desktop_available(&missing, false));
    }
}
