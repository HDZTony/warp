use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::bubble::format_message_time_pub;
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::StatusTone;
use crate::ui::icons;
use crate::ui::panel_primitives::{chat_item_active_bg, chat_search_pill, chat_sidebar_search_bg, tg_avatar};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_conversations, chat_start_conversation, ChatConversationDto,
    StartChatConversationParams,
};
use wormhole_desktop_core::cluster_commands::{cluster_status, ClusterStatusDto};
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

use std::collections::BTreeMap;

pub const TG_SIDEBAR_AVATAR: f32 = 46.0;
/// Inner content width inside sidebar item horizontal padding (12px × 2 in 300px column).
const CHAT_ITEM_INNER_WIDTH: f32 = 276.0;

#[derive(Debug, Clone)]
pub enum ChatSidebarAction {
    Select(String),
    SearchEdit(TextFieldEditAction),
    FocusSearch,
    ActivateSearch,
}

#[derive(Debug, Clone)]
pub enum ChatSidebarEvent {
    Selected(String),
}

struct SidebarRow {
    id: String,
    title: String,
    preview: String,
    time: String,
    online: bool,
    unread: u32,
}

pub struct ChatSidebarView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    rows: Vec<SidebarRow>,
    conversations: Vec<ChatConversationDto>,
    cluster: Option<ClusterStatusDto>,
    remarks: BTreeMap<String, String>,
    selecting: Option<String>,
    search: String,
    search_field: TextFieldState,
    search_focused: bool,
    caret_blink: CaretBlink,
    status: String,
    scroll: ClippedScrollStateHandle,
}

impl ChatSidebarView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            selection,
            shell_state,
            font,
            rows: Vec::new(),
            conversations: Vec::new(),
            cluster: None,
            remarks: BTreeMap::new(),
            selecting: None,
            search: String::new(),
            search_field: TextFieldState::new(),
            search_focused: false,
            caret_blink: CaretBlink::new(),
            status: String::new(),
            scroll: ClippedScrollStateHandle::new(),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let cfg = chat_config(app, &state).await;
                let list = chat_list_conversations(app, &state).await;
                let cluster = cluster_status(&state).await;
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                (cfg, list, cluster, remarks)
            },
            |view, output, ctx| {
                let (cfg, list, cluster, remarks) = output;
                if let Ok(c) = cfg {
                    view.status = c.display_name;
                }
                let conversations = list.unwrap_or_default();
                view.conversations = conversations.clone();
                view.cluster = cluster.ok();
                view.remarks = remarks;
                view.rows = view.build_rows(conversations, view.cluster.as_ref());
                ctx.notify();
            },
        );
    }

    fn apply_selection(&mut self, conv_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut guard) = self.selection.lock() {
            *guard = Some(conv_id.clone());
        }
        self.selecting = None;
        ctx.emit(ChatSidebarEvent::Selected(conv_id));
        ctx.notify();
    }

    fn start_conversation_for_peer(
        &mut self,
        peer: String,
        display_name: Option<String>,
        bootstrap_addrs: Vec<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.selecting = Some(peer.clone());
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let params = StartChatConversationParams {
                    backend: None,
                    peer: Some(peer.clone()),
                    peer_endpoint: None,
                    peer_display_name: display_name,
                    peer_bootstrap_addrs: bootstrap_addrs,
                };
                chat_start_conversation(app, &state, params).await
            },
            |view, output, ctx| {
                view.selecting = None;
                match output {
                    Ok(conv) => {
                        view.apply_selection(conv.id, ctx);
                        view.refresh(ctx);
                    }
                    Err(err) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(format!("无法开始会话: {err}"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn row_is_active(&self, row: &SidebarRow, selected: Option<&str>) -> bool {
        let Some(selected) = selected else {
            return false;
        };
        if row.id == selected {
            return true;
        }
        self.conversations.iter().any(|conv| {
            conv.id == selected && (conv.peer_endpoint == row.id || conv.id == row.id)
        })
    }

    fn build_rows(
        &self,
        conversations: Vec<ChatConversationDto>,
        cluster: Option<&ClusterStatusDto>,
    ) -> Vec<SidebarRow> {
        let mut rows = Vec::new();
        for conv in conversations {
            let remark = cluster.and_then(|c| {
                c.nodes.iter().find_map(|node| {
                    if node.chat_endpoint_id.as_deref() == Some(conv.peer_endpoint.as_str())
                        || node.node_id == conv.peer_endpoint
                    {
                        self.remarks.get(&node.node_id).map(String::as_str)
                    } else {
                        None
                    }
                })
            });
            let fallback = conv
                .title
                .clone()
                .or(conv.peer_display_name.clone())
                .unwrap_or_else(|| "未知设备".to_string());
            let title = display_name_with_remark(remark, || fallback);
            let preview = conv
                .peer_display_name
                .clone()
                .unwrap_or_else(|| "等待消息…".to_string());
            let time = conv
                .last_message_at
                .map(format_message_time_pub)
                .unwrap_or_default();
            rows.push(SidebarRow {
                id: conv.id,
                title,
                preview,
                time,
                online: true,
                unread: 0,
            });
        }
        if rows.is_empty() {
            if let Some(cluster) = cluster {
                for node in &cluster.nodes {
                    let title = display_name_with_remark(
                        self.remarks.get(&node.node_id).map(String::as_str),
                        || format!("{} · {}", node.os, node.hostname),
                    );
                    let id = node
                        .chat_endpoint_id
                        .clone()
                        .unwrap_or_else(|| node.node_id.clone());
                    rows.push(SidebarRow {
                        id,
                        title,
                        preview: if node.online {
                            "在线 · 等待消息…".into()
                        } else {
                            "离线".into()
                        },
                        time: String::new(),
                        online: node.online,
                        unread: 0,
                    });
                }
            }
        }
        rows
    }

    fn filtered_rows(&self) -> Vec<&SidebarRow> {
        let q = self.search.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| {
                q.is_empty()
                    || row.title.to_lowercase().contains(&q)
                    || row.preview.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn avatar_initials(title: &str) -> String {
        let compact: String = title
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(2)
            .collect();
        if compact.is_empty() {
            "WH".into()
        } else {
            compact.to_uppercase()
        }
    }

    fn chat_item(&self, row: &SidebarRow, selected: bool) -> Box<dyn Element> {
        let id = row.id.clone();
        let mut top = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        top.add_child(
            ui_text::chat_sidebar_name(row.title.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if !row.time.is_empty() {
            top.add_child(
                ui_text::chat_sidebar_time(row.time.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        let preview_color = if selected {
            theme::text()
        } else {
            theme::muted()
        };
        let mut preview_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        preview_row.add_child(
            ui_text::chat_preview(row.preview.clone(), self.font)
                .with_color(preview_color)
                .finish(),
        );
        if row.unread > 0 {
            preview_row.add_child(
                Container::new(
                    Align::new(
                        ui_text::device_meta(row.unread.to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_uniform_padding(4.0)
                .with_background(theme::accent_cool())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                .finish(),
            );
        }

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(top.finish());
        col.add_child(
            Container::new(preview_row.finish())
                .with_margin_top(3.0)
                .finish(),
        );

        let row_body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(tg_avatar(
                Self::avatar_initials(&row.title),
                self.font,
                TG_SIDEBAR_AVATAR,
            ))
            .with_child(
                Container::new(col.finish())
                    .with_margin_left(10.0)
                    .finish(),
            )
            .finish();

        let interactive = EventHandler::new(
            ConstrainedBox::new(row_body)
                .with_width(CHAT_ITEM_INNER_WIDTH)
                .finish(),
        )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::Select(id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish();

        Container::new(interactive)
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(9.0)
            .with_padding_bottom(9.0)
            .with_background(if selected {
                chat_item_active_bg()
            } else {
                pathfinder_color::ColorU::transparent_black()
            })
            .finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let search_focused = self.search_focused;
        let draft = self.search.clone();
        let marked = self.search_field.marked_text.clone();
        let field = render_search_field_with_caret(
            &draft,
            &marked,
            "搜索终端…",
            self.font,
            search_focused,
            false,
            self.caret_blink.visible,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ChatSidebarAction::SearchEdit(action));
        })
        .focused(search_focused)
        .ime_preedit(!marked.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "tab" || keystroke.key == "escape" {
                ctx.dispatch_typed_action(ChatSidebarAction::FocusSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ChatSidebarAction::ActivateSearch);
        });

        let border_color = if search_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_horizontal_margin(2.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();

        chat_search_pill(
            row,
            chat_sidebar_search_bg(),
            border_color,
            7.0,
            10.0,
            999.0,
        )
    }
}

impl Entity for ChatSidebarView {
    type Event = ChatSidebarEvent;
}

impl View for ChatSidebarView {
    fn ui_name() -> &'static str {
        "ChatSidebarView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let rows = self.filtered_rows();

        let mut list = Flex::column().with_main_axis_size(MainAxisSize::Min);
        if rows.is_empty() {
            list.add_child(
                Container::new(
                    ui_text::body(
                        if self.search.is_empty() {
                            if self.status.is_empty() {
                                "暂无会话".to_string()
                            } else {
                                format!("暂无会话 · {}", self.status)
                            }
                        } else {
                            "无匹配结果".to_string()
                        },
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_uniform_padding(12.0)
                .finish(),
            );
        } else {
            for row in rows {
                let active = self.row_is_active(row, selected.as_deref());
                list.add_child(self.chat_item(row, active));
            }
        }

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .with_child(
                            Expanded::new(
                                1.0,
                                ConstrainedBox::new(self.search_box())
                                    .with_width(CHAT_ITEM_INNER_WIDTH)
                                    .finish(),
                            )
                            .finish(),
                        )
                        .finish(),
                )
                    .with_horizontal_padding(12.0)
                    .with_vertical_padding(10.0)
                    .with_background(theme::panel())
                    .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                    .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(
                        ClippedScrollable::vertical(
                            self.scroll.clone(),
                            Container::new(list.finish())
                                .with_vertical_padding(4.0)
                                .finish(),
                            ScrollbarWidth::Auto,
                            Fill::None,
                            Fill::None,
                            Fill::None,
                        )
                        .finish(),
                    )
                    .with_background(theme::panel())
                    .with_border(Border::right(1.0).with_border_fill(theme::border()))
                    .finish(),
                )
                .finish(),
            )
            .finish()
    }
}

impl TypedActionView for ChatSidebarView {
    type Action = ChatSidebarAction;

    fn handle_action(&mut self, action: &ChatSidebarAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatSidebarAction::Select(id) => {
                if self.selecting.is_some() {
                    return;
                }
                match resolve_select_target(id, &self.conversations, self.cluster.as_ref()) {
                    SelectTarget::Conversation(conv_id) => {
                        self.apply_selection(conv_id, ctx);
                    }
                    SelectTarget::StartWithPeer {
                        peer,
                        display_name,
                        bootstrap_addrs,
                    } => {
                        self.start_conversation_for_peer(peer, display_name, bootstrap_addrs, ctx);
                    }
                }
            }
            ChatSidebarAction::FocusSearch => {
                self.search_focused = !self.search_focused;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatSidebarAction::ActivateSearch => {
                if !self.search_focused {
                    self.search_focused = true;
                    sync_caret_blink(self, ctx);
                    ctx.notify();
                }
            }
            ChatSidebarAction::SearchEdit(edit) => {
                self.search_field.apply(&mut self.search, edit);
                self.search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectTarget {
    Conversation(String),
    StartWithPeer {
        peer: String,
        display_name: Option<String>,
        bootstrap_addrs: Vec<String>,
    },
}

fn resolve_select_target(
    id: &str,
    conversations: &[ChatConversationDto],
    cluster: Option<&ClusterStatusDto>,
) -> SelectTarget {
    if conversations.iter().any(|conv| conv.id == id) {
        return SelectTarget::Conversation(id.to_string());
    }
    if let Some(conv) = conversations
        .iter()
        .find(|conv| conv.peer_endpoint == id)
    {
        return SelectTarget::Conversation(conv.id.clone());
    }
    if let Some(cluster) = cluster {
        if let Some(node) = cluster.nodes.iter().find(|node| {
            node.chat_endpoint_id.as_deref() == Some(id) || node.node_id == id
        }) {
            let peer = node
                .chat_endpoint_id
                .clone()
                .unwrap_or_else(|| node.node_id.clone());
            let display_name = format!("{} · {}", node.os, node.hostname);
            return SelectTarget::StartWithPeer {
                peer,
                display_name: Some(display_name),
                bootstrap_addrs: node.chat_bootstrap_addrs.clone(),
            };
        }
    }
    SelectTarget::StartWithPeer {
        peer: id.to_string(),
        display_name: None,
        bootstrap_addrs: Vec::new(),
    }
}

impl CaretBlinkHost for ChatSidebarView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use wormhole_desktop_core::cluster_commands::{ClusterNodeDto, ClusterStatusDto};

    fn sample_conv(id: &str, peer: &str) -> ChatConversationDto {
        ChatConversationDto {
            id: id.into(),
            backend: "wormhole".into(),
            kind: "direct".into(),
            title: Some("Peer".into()),
            cluster_id: None,
            members: Vec::new(),
            created_by: None,
            membership_version: 0,
            peer_endpoint: peer.into(),
            peer_display_name: None,
            peer_bootstrap_addrs: Vec::new(),
            doc_ticket: String::new(),
            created_at: 0,
            last_message_at: None,
        }
    }

    #[test]
    fn resolve_select_target_uses_existing_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("conv-hash", &convs, None);
        assert_eq!(
            target,
            SelectTarget::Conversation("conv-hash".to_string())
        );
    }

    #[test]
    fn resolve_select_target_maps_peer_endpoint_to_conv_id() {
        let convs = vec![sample_conv("conv-hash", "peer-endpoint")];
        let target = resolve_select_target("peer-endpoint", &convs, None);
        assert_eq!(
            target,
            SelectTarget::Conversation("conv-hash".to_string())
        );
    }

    #[test]
    fn resolve_select_target_cluster_node_starts_conversation() {
        let cluster = ClusterStatusDto {
            configured: true,
            cluster_id: None,
            clusters: Vec::new(),
            joined_at: None,
            device_id: None,
            local_node_id: String::new(),
            transport: String::new(),
            nodes: vec![ClusterNodeDto {
                node_id: "node-1".into(),
                device_id: None,
                chat_endpoint_id: Some("chat-endpoint".into()),
                chat_bootstrap_addrs: vec!["bootstrap-1".into()],
                hostname: "host".into(),
                os: "Windows".into(),
                roles: Vec::new(),
                online: true,
                presence_status: String::new(),
                cpu_cores: 0,
                memory_total: 0,
                storage_total: 0,
                storage_free: 0,
                billing_node_score: None,
                billing_expired: false,
                role: String::new(),
                removable: false,
                revoked: false,
                server_member_confirmed: false,
            }],
            storage_volumes: Vec::new(),
            normal_replica_target: 0,
            photo_video_replica_target: 0,
            build_cache_replica_target: 0,
            normal_replica_degraded: false,
            photo_video_replica_degraded: false,
            syncing: false,
            auth_required: false,
            device_bootstrap_required: false,
            device_bootstrap_error: None,
        };
        let target = resolve_select_target("chat-endpoint", &[], Some(&cluster));
        assert_eq!(
            target,
            SelectTarget::StartWithPeer {
                peer: "chat-endpoint".into(),
                display_name: Some("Windows · host".into()),
                bootstrap_addrs: vec!["bootstrap-1".into()],
            }
        );
    }
}
