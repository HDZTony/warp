//! Calls modal — design §7.6 / desktop-current.html `#chat-calls-panel`.

use std::collections::HashSet;

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::chat::voice_call_ui;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{tg_avatar, StatusTone, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::call_history::{
    call_history_clear, call_history_list, call_history_put, expires_at_from_end, new_call_id,
    CallHistoryDirection, CallHistoryKind, CallHistoryListDto, CallHistoryParticipant,
    CallHistoryRecord, CallHistoryStatus,
};
use wormhole_desktop_core::chat_commands::{
    chat_create_cluster_group, chat_list_conversations, chat_send_message, chat_start_conversation,
    ChatConversationDto, ChatGroupMemberDto, CreateClusterGroupParams, SendChatMessageParams,
    StartChatConversationParams,
};
use wormhole_desktop_core::chat_contacts::{
    aggregate_cluster_contacts_by_account, chat_contacts_list_manual, merge_contact_rows,
    ContactDto, ContactSource,
};
use wormhole_desktop_core::chat_rtc_call::{
    load_video_device_prefs, load_voice_device_prefs, save_video_device_prefs,
    save_voice_device_prefs, VideoDevicePrefs, VoiceDevicePrefs,
};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};
use wormhole_desktop_core::video_chat_commands::{
    video_chat_create_room, video_chat_join, video_room_invite_body, VideoChatCreateRoomParams,
    VideoChatJoinParams,
};

const DIALOG_WIDTH: f32 = 420.0;
const DIALOG_HEIGHT: f32 = 560.0;
const VOICE_MAX_OTHERS: usize = 1;
const VIDEO_MAX_OTHERS: usize = 7;

#[derive(Debug, Clone)]
pub enum CallsPanelAction {
    Close,
    OpenPicker,
    BackOverview,
    SetCallType(bool),
    ToggleParticipant(String),
    PrepareCall,
    ToggleMoreMenu,
    OpenSettings,
    OpenPrivacy,
    OpenConfirmClear,
    BackFromSub,
    ConfirmClear,
    ToggleMicMuted,
    ToggleCameraMuted,
    Refresh,
    RejoinHistory { call_id: String },
}

#[derive(Debug, Clone)]
pub enum CallsPanelEvent {
    OpenConversation(String),
    OpenVideoViewerUrl(String),
    OpenLiveViewer { peer: String, title: String },
    Closed,
}

pub struct CallsPanelView {
    core: CoreHandle,
    shell_state: SharedChatShellState,
    font: FamilyId,
    history: Vec<CallHistoryRecord>,
    contacts: Vec<ContactDto>,
    conversations: Vec<ChatConversationDto>,
    cluster_id: Option<String>,
    call_type_voice: bool,
    selected: HashSet<String>,
    voice_prefs: VoiceDevicePrefs,
    video_prefs: VideoDevicePrefs,
    status: String,
    status_tone: StatusTone,
    scroll: ClippedScrollStateHandle,
    last_open: bool,
    clearing: bool,
}

impl CallsPanelView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let data_dir = core.runtime().state.data_dir.clone();
        let mut view = Self {
            core,
            shell_state,
            font,
            history: Vec::new(),
            contacts: Vec::new(),
            conversations: Vec::new(),
            cluster_id: None,
            call_type_voice: true,
            selected: HashSet::new(),
            voice_prefs: load_voice_device_prefs(&data_dir),
            video_prefs: load_video_device_prefs(&data_dir),
            status: String::new(),
            status_tone: StatusTone::Neutral,
            scroll: ClippedScrollStateHandle::new(),
            last_open: false,
            clearing: false,
        };
        view.poll_open(ctx);
        view
    }

    fn subview(&self) -> String {
        self.shell_state
            .lock()
            .map(|s| s.calls_subview.clone())
            .unwrap_or_else(|_| "overview".into())
    }

    fn menu_open(&self) -> bool {
        self.shell_state
            .lock()
            .map(|s| s.calls_menu_open)
            .unwrap_or(false)
    }

    fn poll_open(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(220)).await;
            },
            |view, _, ctx| {
                let open = view
                    .shell_state
                    .lock()
                    .map(|s| s.calls_open)
                    .unwrap_or(false);
                if open && !view.last_open {
                    view.last_open = true;
                    view.reload(ctx);
                } else if !open {
                    view.last_open = false;
                }
                view.poll_open(ctx);
                ctx.notify();
            },
        );
    }

    fn reload(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                let history = call_history_list(&state)
                    .await
                    .unwrap_or(CallHistoryListDto {
                        records: Vec::new(),
                    });
                let manual = chat_contacts_list_manual(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                let cluster = cluster_status_hud(&state).await.ok();
                let conversations = {
                    let app = runtime.ctx.as_ref();
                    chat_list_conversations(app, &state)
                        .await
                        .unwrap_or_default()
                };
                let local_endpoint = state
                    .manager
                    .node_if_ready()
                    .map(|n| n.endpoint_id().to_string())
                    .unwrap_or_default();
                let mut cluster_id = None;
                let cluster_rows = if let Some(status) = cluster {
                    cluster_id = status.cluster_id.clone();
                    let local_user_id = status
                        .nodes
                        .iter()
                        .find(|node| node.node_id == status.local_node_id)
                        .and_then(|node| node.user_id.clone());
                    aggregate_cluster_contacts_by_account(
                        &status.nodes,
                        &status.local_node_id,
                        local_user_id.as_deref(),
                        &local_endpoint,
                        &remarks,
                    )
                } else {
                    Vec::new()
                };
                let contacts = merge_contact_rows(cluster_rows, manual)
                    .into_iter()
                    .filter(|c| c.can_chat)
                    .collect::<Vec<_>>();
                (
                    history.records,
                    contacts,
                    conversations,
                    cluster_id,
                    load_voice_device_prefs(&state.data_dir),
                    load_video_device_prefs(&state.data_dir),
                )
            },
            |view, packed, ctx| {
                let (history, contacts, conversations, cluster_id, voice, video) = packed;
                view.history = history;
                view.contacts = contacts;
                view.conversations = conversations;
                view.cluster_id = cluster_id;
                view.voice_prefs = voice;
                view.video_prefs = video;
                view.status.clear();
                ctx.notify();
            },
        );
    }

    fn max_others(&self) -> usize {
        if self.call_type_voice {
            VOICE_MAX_OTHERS
        } else {
            VIDEO_MAX_OTHERS
        }
    }

    fn can_prepare(&self) -> bool {
        let n = self.selected.len();
        if self.call_type_voice {
            n == 1
        } else {
            (1..=VIDEO_MAX_OTHERS).contains(&n)
        }
    }

    fn prepare_call(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.can_prepare() {
            self.status = if self.call_type_voice {
                "语音通话请选择 1 位联系人".into()
            } else {
                "视频通话请选择 1–7 位联系人".into()
            };
            self.status_tone = StatusTone::Warn;
            ctx.notify();
            return;
        }
        let selected: Vec<ContactDto> = self
            .contacts
            .iter()
            .filter(|c| self.selected.contains(&c.id))
            .cloned()
            .collect();
        if self.call_type_voice {
            self.start_voice(selected, ctx);
        } else {
            self.start_video(selected, ctx);
        }
    }

    fn start_voice(&mut self, selected: Vec<ContactDto>, ctx: &mut ViewContext<Self>) {
        let Some(peer) = selected.into_iter().next() else {
            return;
        };
        let core = self.core.clone();
        let peer_id = peer.wormhole_id.clone();
        let peer_name = peer.display_name.clone();
        let prefs = self.voice_prefs.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let conv = chat_start_conversation(
                    app,
                    &runtime.state,
                    StartChatConversationParams {
                        backend: None,
                        peer: Some(peer_id.clone()),
                        peer_endpoint: Some(peer_id.clone()),
                        peer_display_name: Some(peer_name.clone()),
                        peer_bootstrap_addrs: Vec::new(),
                    },
                )
                .await?;
                let status = voice_call_ui::invite_with_prefs(&core, &conv.id, &prefs).await?;
                Ok::<_, String>((conv.id, status, peer_id))
            },
            |view, result, ctx| match result {
                Ok((conv_id, status, peer)) => {
                    voice_call_ui::apply_voice_status(&view.shell_state, &status);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.close_calls();
                        state.show_toast("正在发起语音通话…", StatusTone::Success);
                    }
                    ctx.emit(CallsPanelEvent::OpenConversation(conv_id));
                    if status.phase == "active" && status.peer_live && !peer.is_empty() {
                        ctx.emit(CallsPanelEvent::OpenLiveViewer {
                            peer,
                            title: "语音通话".into(),
                        });
                    }
                    ctx.notify();
                }
                Err(err) => {
                    view.status = err;
                    view.status_tone = StatusTone::Danger;
                    ctx.notify();
                }
            },
        );
    }

    fn start_video(&mut self, selected: Vec<ContactDto>, ctx: &mut ViewContext<Self>) {
        match video_call_route_for_selection(selected.len()) {
            Some("p2p") => self.start_p2p_video(selected, ctx),
            Some("sfu") => self.start_sfu_video(selected, ctx),
            _ => {
                self.status = "请先选择联系人".into();
                self.status_tone = StatusTone::Warn;
                ctx.notify();
            }
        }
    }

    fn start_p2p_video(&mut self, selected: Vec<ContactDto>, ctx: &mut ViewContext<Self>) {
        let Some(peer) = selected.into_iter().next() else {
            return;
        };
        let core = self.core.clone();
        let peer_id = peer.wormhole_id.clone();
        let peer_name = peer.display_name.clone();
        let prefs = self.video_prefs.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let conv = chat_start_conversation(
                    app,
                    &runtime.state,
                    StartChatConversationParams {
                        backend: None,
                        peer: Some(peer_id.clone()),
                        peer_endpoint: Some(peer_id.clone()),
                        peer_display_name: Some(peer_name.clone()),
                        peer_bootstrap_addrs: Vec::new(),
                    },
                )
                .await?;
                let status =
                    voice_call_ui::invite_video_with_prefs(&core, &conv.id, &prefs).await?;
                Ok::<_, String>((conv.id, status, peer_id, peer_name))
            },
            |view, result, ctx| match result {
                Ok((conv_id, status, peer, name)) => {
                    voice_call_ui::apply_voice_status(&view.shell_state, &status);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.close_calls();
                        state.show_toast("正在发起视频通话…", StatusTone::Success);
                    }
                    ctx.emit(CallsPanelEvent::OpenConversation(conv_id));
                    if status.phase == "active" && status.peer_live && !peer.is_empty() {
                        ctx.emit(CallsPanelEvent::OpenLiveViewer {
                            peer,
                            title: format!("视频 · {name}"),
                        });
                    }
                    ctx.notify();
                }
                Err(err) => {
                    let (text, tone) = voice_call_ui::video_error_toast(&err);
                    view.status = text;
                    view.status_tone = tone;
                    ctx.notify();
                }
            },
        );
    }

    fn start_sfu_video(&mut self, selected: Vec<ContactDto>, ctx: &mut ViewContext<Self>) {
        let Some(cluster_id) = self.cluster_id.clone() else {
            self.status = "群视频需要已加入家庭集群".into();
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return;
        };
        if selected.len() < 2 {
            self.status = "群视频请选择至少 2 位联系人（1 人请用一对一视频）".into();
            self.status_tone = StatusTone::Warn;
            ctx.notify();
            return;
        }
        let endpoints: HashSet<String> = selected.iter().map(|c| c.wormhole_id.clone()).collect();
        let existing = self.conversations.iter().find(|conv| {
            conv.kind == "cluster_group"
                && conv.cluster_id.as_deref() == Some(cluster_id.as_str())
                && {
                    let members: HashSet<_> = conv
                        .members
                        .iter()
                        .filter(|m| !m.is_local)
                        .map(|m| m.endpoint.clone())
                        .collect();
                    !endpoints.is_empty() && endpoints.is_subset(&members)
                }
        });
        let core = self.core.clone();
        let selected_clone = selected.clone();
        let existing_id = existing.map(|c| c.id.clone());
        let cluster_snapshot = self
            .contacts
            .iter()
            .filter(|c| endpoints.contains(&c.wormhole_id))
            .cloned()
            .collect::<Vec<_>>();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let conv_id = if let Some(id) = existing_id {
                    id
                } else {
                    let members: Vec<ChatGroupMemberDto> = selected_clone
                        .iter()
                        .map(|c| ChatGroupMemberDto {
                            node_id: c.wormhole_id.clone(),
                            endpoint: c.wormhole_id.clone(),
                            display_name: Some(c.display_name.clone()),
                            bootstrap_addrs: Vec::new(),
                            is_local: false,
                        })
                        .collect();
                    let title = format!("群视频（{} 人）", selected_clone.len() + 1);
                    let conv = chat_create_cluster_group(
                        app,
                        &runtime.state,
                        CreateClusterGroupParams {
                            title,
                            cluster_id: cluster_id.clone(),
                            members,
                            default_group: false,
                        },
                    )
                    .await?;
                    conv.id
                };
                let room = video_chat_create_room(
                    &runtime.state,
                    VideoChatCreateRoomParams {
                        cluster_id: cluster_id.clone(),
                        group_conv_id: conv_id.clone(),
                        display_name: None,
                    },
                )
                .await?;
                let join = video_chat_join(
                    &runtime.state,
                    VideoChatJoinParams {
                        room_id: room.room_id.clone(),
                        display_name: None,
                    },
                )
                .await?;
                let body = video_room_invite_body(&room.room_id, &cluster_id, &conv_id);
                let _ = chat_send_message(
                    app,
                    &runtime.state,
                    SendChatMessageParams {
                        conv_id: conv_id.clone(),
                        body,
                        backend: None,
                        peer_bootstrap_addrs: Vec::new(),
                        sticker: None,
                        attachments: Vec::new(),
                    },
                )
                .await;
                let now = unix_now();
                let _ = call_history_put(
                    &runtime.state,
                    CallHistoryRecord {
                        call_id: new_call_id(),
                        kind: CallHistoryKind::GroupVideo,
                        direction: CallHistoryDirection::Outgoing,
                        status: CallHistoryStatus::Active,
                        participants: cluster_snapshot
                            .into_iter()
                            .map(|c| CallHistoryParticipant {
                                endpoint_id: c.wormhole_id,
                                display_name: c.display_name,
                            })
                            .collect(),
                        started_at: now,
                        ended_at: None,
                        expires_at: expires_at_from_end(now),
                        room_id: Some(room.room_id),
                        conv_id: Some(conv_id.clone()),
                    },
                )
                .await;
                Ok::<_, String>((conv_id, join.open_url))
            },
            |view, result, ctx| match result {
                Ok((conv_id, url)) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.close_calls();
                        state.show_toast("已打开群视频", StatusTone::Success);
                    }
                    ctx.emit(CallsPanelEvent::OpenConversation(conv_id));
                    ctx.emit(CallsPanelEvent::OpenVideoViewerUrl(url));
                    ctx.notify();
                }
                Err(err) => {
                    let (text, tone) = voice_call_ui::video_error_toast(&err);
                    view.status = text;
                    view.status_tone = tone;
                    ctx.notify();
                }
            },
        );
    }

    fn clear_history(&mut self, ctx: &mut ViewContext<Self>) {
        if self.clearing {
            return;
        }
        self.clearing = true;
        self.status = "正在清除我的记录…".into();
        self.status_tone = StatusTone::Neutral;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                call_history_clear(&runtime.state).await
            },
            |view, result, ctx| {
                view.clearing = false;
                match result {
                    Ok(n) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.calls_subview = "overview".into();
                            state.calls_menu_open = false;
                            state.show_toast(format!("已清除我的记录（{n} 条）"), StatusTone::Success);
                        }
                        view.status.clear();
                        view.reload(ctx);
                    }
                    Err(err) => {
                        view.status = format!("清除失败，可重试: {err}");
                        view.status_tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn rejoin_history(&mut self, call_id: &str, ctx: &mut ViewContext<Self>) {
        let Some(rec) = self.history.iter().find(|r| r.call_id == call_id).cloned() else {
            return;
        };
        match rec.kind {
            CallHistoryKind::GroupVideo => {
                let Some(room_id) = rec
                    .room_id
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .cloned()
                else {
                    self.status = "该群视频记录没有房间号，无法重入".into();
                    self.status_tone = StatusTone::Warn;
                    ctx.notify();
                    return;
                };
                if !matches!(rec.status, CallHistoryStatus::Active | CallHistoryStatus::Ringing) {
                    self.status = "仅进行中的群视频可重新加入".into();
                    self.status_tone = StatusTone::Warn;
                    ctx.notify();
                    return;
                }
                let conv_id = rec.conv_id.clone();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        video_chat_join(
                            &runtime.state,
                            VideoChatJoinParams {
                                room_id,
                                display_name: None,
                            },
                        )
                        .await
                    },
                    move |_view, result, ctx| match result {
                        Ok(join) => {
                            if let Ok(mut state) = _view.shell_state.lock() {
                                state.close_calls();
                                state.show_toast("已重新加入群视频", StatusTone::Success);
                            }
                            if let Some(conv) = conv_id {
                                ctx.emit(CallsPanelEvent::OpenConversation(conv));
                            }
                            ctx.emit(CallsPanelEvent::OpenVideoViewerUrl(join.open_url));
                            ctx.notify();
                        }
                        Err(err) => {
                            let (text, tone) = voice_call_ui::video_error_toast(&err);
                            _view.status = text;
                            _view.status_tone = tone;
                            ctx.notify();
                        }
                    },
                );
            }
            CallHistoryKind::Voice | CallHistoryKind::Video => {
                let Some(conv_id) = rec.conv_id.filter(|s| !s.is_empty()) else {
                    self.status = "该通话记录没有会话，无法打开".into();
                    self.status_tone = StatusTone::Warn;
                    ctx.notify();
                    return;
                };
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_calls();
                    state.show_toast("已打开会话，可从顶栏重拨", StatusTone::Success);
                }
                ctx.emit(CallsPanelEvent::OpenConversation(conv_id));
                ctx.notify();
            }
        }
    }

    fn persist_prefs(&self) {
        let data_dir = self.core.runtime().state.data_dir.clone();
        save_voice_device_prefs(&data_dir, &self.voice_prefs);
        save_video_device_prefs(&data_dir, &self.video_prefs);
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn open_external_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("不支持打开该链接".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持自动打开链接".into())
}

impl Entity for CallsPanelView {
    type Event = CallsPanelEvent;
}

impl View for CallsPanelView {
    fn ui_name() -> &'static str {
        "CallsPanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|s| s.calls_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }

        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 140))
                .finish(),
        )
        .with_automation_label("关闭通话")
        .with_automation_id("chat:calls_scrim")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(CallsPanelAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let dialog = ConstrainedBox::new(self.dialog_body())
            .with_width(DIALOG_WIDTH)
            .with_height(DIALOG_HEIGHT)
            .finish();
        let dialog = EventHandler::new(dialog)
            .with_automation_label("通话")
            .with_automation_id("chat:calls_dialog")
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish();
        let centered = Align::new(dialog)
            .finish();

        Stack::new()
            .with_child(scrim)
            .with_child(centered)
            .finish()
    }
}

impl CallsPanelView {
    fn dialog_body(&self) -> Box<dyn Element> {
        let sub = self.subview();
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        col.add_child(self.head_bar(&sub));

        let body: Box<dyn Element> = match sub.as_str() {
            "picker" => self.picker_body(),
            "settings" => self.settings_body(),
            "privacy" => self.privacy_body(),
            "confirm" => self.confirm_body(),
            _ => self.overview_body(),
        };
        col.add_child(Expanded::new(1.0, body).finish());

        if !self.status.is_empty() {
            col.add_child(
                Container::new(
                    ui_text::chat_preview(self.status.clone(), self.font)
                        .with_color(match self.status_tone {
                            StatusTone::Danger => theme::danger(),
                            StatusTone::Success => theme::accent_cool(),
                            StatusTone::Warn => theme::warn(),
                            _ => theme::muted(),
                        })
                        .finish(),
                )
                .with_padding_left(16.0)
                .with_padding_right(16.0)
                .with_padding_bottom(8.0)
                .finish(),
            );
        }

        let panel = Container::new(col.finish())
            .with_background(theme::panel())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 2.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish();

        if self.menu_open() && sub == "overview" {
            Stack::new()
                .with_child(panel)
                .with_child(self.more_menu_overlay())
                .finish()
        } else {
            panel
        }
    }

    fn head_bar(&self, sub: &str) -> Box<dyn Element> {
        let title = match sub {
            "picker" => "开始新通话",
            "settings" => "通话设置",
            "privacy" => "通话隐私",
            "confirm" => "清除我的记录",
            _ => "通话",
        };
        let mut row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        if sub != "overview" {
            row.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body("←".to_string(), self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .with_uniform_padding(8.0)
                    .finish(),
                )
                .with_automation_label("返回")
                .with_automation_id("chat:calls_back")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(CallsPanelAction::BackFromSub);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            );
        }
        row.add_child(Expanded::new(1.0, ui_text::body(title.to_string(), self.font)
                .with_color(theme::text())
                .finish()).finish());
        if sub == "overview" {
            row.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body("⋯".to_string(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_uniform_padding(8.0)
                    .finish(),
                )
                .with_automation_label("更多")
                .with_automation_id("chat:calls_more")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(CallsPanelAction::ToggleMoreMenu);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            );
        }
        row.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("关闭".to_string(), self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(8.0)
                .finish(),
            )
            .with_automation_label("关闭")
            .with_automation_id("chat:calls_close")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(CallsPanelAction::Close);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        Container::new(row.finish())
            .with_padding_left(14.0)
            .with_padding_right(10.0)
            .with_padding_top(12.0)
            .with_padding_bottom(10.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn overview_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            Container::new(
                EventHandler::new(
                    Container::new(
                        Flex::column()
                            .with_child(
                                ui_text::body("开始新通话".to_string(), self.font)
                                    .with_color(theme::text())
                                    .finish(),
                            )
                            .with_child(
                                ui_text::chat_preview(
                                    "选择联系人发起语音或视频".to_string(),
                                    self.font,
                                )
                                .with_color(theme::muted())
                                .finish(),
                            )
                            .finish(),
                    )
                    .with_uniform_padding(14.0)
                    .with_background(theme::accent_cool_bg(20))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                    .finish(),
                )
                .with_automation_label("开始新通话")
                .with_automation_id("chat:calls_start_new")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(CallsPanelAction::OpenPicker);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_top(12.0)
            .with_padding_bottom(8.0)
            .finish(),
        );

        if self.history.is_empty() {
            col.add_child(
                Expanded::new(
                    1.0,
                    Align::new(
                        ui_text::chat_preview("暂无通话记录".to_string(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
            );
        } else {
            let mut list = Flex::column().with_main_axis_size(MainAxisSize::Min);
            for rec in &self.history {
                list.add_child(self.history_row(rec));
            }
            col.add_child(
                Expanded::new(
                    1.0,
                    ClippedScrollable::vertical(
                        self.scroll.clone(),
                        list.finish(),
                        ScrollbarWidth::Auto,
                        Fill::None,
                        Fill::None,
                        Fill::None,
                    )
                    .finish(),
                )
                .finish(),
            );
        }
        col.finish()
    }

    fn history_row(&self, rec: &CallHistoryRecord) -> Box<dyn Element> {
        let names = if rec.participants.is_empty() {
            "未知".to_string()
        } else {
            rec.participants
                .iter()
                .map(|p| {
                    if p.display_name.is_empty() {
                        p.endpoint_id.clone()
                    } else {
                        p.display_name.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("、")
        };
        let kind = match rec.kind {
            CallHistoryKind::Voice => "语音",
            CallHistoryKind::Video => "视频",
            CallHistoryKind::GroupVideo => "群视频",
        };
        let dir = match rec.direction {
            CallHistoryDirection::Outgoing => "呼出",
            CallHistoryDirection::Incoming => "呼入",
        };
        let status = match rec.status {
            CallHistoryStatus::Ringing => "响铃中",
            CallHistoryStatus::Active => "进行中",
            CallHistoryStatus::Ended => "已结束",
            CallHistoryStatus::Missed => "未接",
            CallHistoryStatus::Declined => "已拒绝",
            CallHistoryStatus::Cancelled => "已取消",
            CallHistoryStatus::Failed => "失败",
        };
        let can_rejoin = match rec.kind {
            CallHistoryKind::GroupVideo => {
                matches!(rec.status, CallHistoryStatus::Active | CallHistoryStatus::Ringing)
                    && rec.room_id.as_ref().is_some_and(|s| !s.is_empty())
            }
            CallHistoryKind::Voice | CallHistoryKind::Video => {
                rec.conv_id.as_ref().is_some_and(|s| !s.is_empty())
            }
        };
        let hint = if can_rejoin {
            match rec.kind {
                CallHistoryKind::GroupVideo => "点击重新加入",
                _ => "点击打开会话",
            }
        } else {
            ""
        };
        let call_id = rec.call_id.clone();
        let row = Container::new(
            Flex::column()
                .with_child(
                    ui_text::body(names, self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_child(
                    ui_text::chat_preview(
                        if hint.is_empty() {
                            format!("{kind} · {dir} · {status}")
                        } else {
                            format!("{kind} · {dir} · {status} · {hint}")
                        },
                        self.font,
                    )
                    .with_color(if can_rejoin {
                        theme::accent_cool()
                    } else {
                        theme::muted()
                    })
                    .finish(),
                )
                .finish(),
        )
        .with_padding_left(16.0)
        .with_padding_right(16.0)
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .finish();
        if !can_rejoin {
            return row;
        }
        EventHandler::new(row)
            .with_automation_label("重新加入")
            .with_automation_id(format!("chat:calls_rejoin:{call_id}"))
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(CallsPanelAction::RejoinHistory {
                    call_id: call_id.clone(),
                });
                DispatchEventResult::StopPropagation
            })
            .finish()
    }

    fn picker_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        let mut type_row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        type_row.add_child(self.type_btn("语音通话", true, self.call_type_voice));
        type_row.add_child(self.type_btn("视频通话", false, !self.call_type_voice));
        col.add_child(
            Container::new(type_row.finish())
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(10.0)
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::chat_preview(
                    if self.call_type_voice {
                        "语音：选择 1 位联系人".into()
                    } else {
                        format!(
                            "视频：最多 {} 位联系人（含自己共 {} 人）",
                            VIDEO_MAX_OTHERS,
                            VIDEO_MAX_OTHERS + 1
                        )
                    },
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_padding_left(16.0)
            .with_padding_top(6.0)
            .with_padding_bottom(4.0)
            .finish(),
        );

        let mut list = Flex::column().with_main_axis_size(MainAxisSize::Min);
        if self.contacts.is_empty() {
            list.add_child(
                Container::new(
                    ui_text::chat_preview("暂无可用联系人".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(16.0)
                .finish(),
            );
        } else {
            for c in &self.contacts {
                list.add_child(self.participant_row(c));
            }
        }
        col.add_child(
            Expanded::new(
                1.0,
                ClippedScrollable::vertical(
                    self.scroll.clone(),
                    list.finish(),
                    ScrollbarWidth::Auto,
                    Fill::None,
                    Fill::None,
                    Fill::None,
                )
                .finish(),
            )
            .finish(),
        );

        let enabled = self.can_prepare();
        col.add_child(
            Container::new(
                EventHandler::new(
                    Container::new(
                        ui_text::body("准备通话".to_string(), self.font)
                            .with_color(if enabled {
                                ColorU::white()
                            } else {
                                theme::muted()
                            })
                            .finish(),
                    )
                    .with_uniform_padding(12.0)
                    .with_background(if enabled {
                        theme::accent_cool()
                    } else {
                        theme::border()
                    })
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                    .finish(),
                )
                .with_automation_label("准备通话")
                .with_automation_id("chat:calls_prepare")
                .on_left_mouse_down(move |ctx, _, _| {
                    if enabled {
                        ctx.dispatch_typed_action(CallsPanelAction::PrepareCall);
                    }
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_bottom(14.0)
            .finish(),
        );
        col.finish()
    }

    fn type_btn(&self, label: &str, voice: bool, pressed: bool) -> Box<dyn Element> {
        let btn_label = label.to_string();
        EventHandler::new(
            Container::new(
                ui_text::body(btn_label.clone(), self.font)
                    .with_color(if pressed {
                        theme::text()
                    } else {
                        theme::muted()
                    })
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(if pressed {
                theme::accent_cool_bg(28)
            } else {
                ColorU::new(0, 0, 0, 0)
            })
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_automation_label(btn_label)
        .with_automation_id(if voice {
            "chat:calls_type_voice"
        } else {
            "chat:calls_type_video"
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(CallsPanelAction::SetCallType(voice));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn participant_row(&self, c: &ContactDto) -> Box<dyn Element> {
        let id = c.id.clone();
        let selected = self.selected.contains(&id);
        let at_limit = !selected && self.selected.len() >= self.max_others();
        let limit_hint = if at_limit {
            if self.call_type_voice {
                "（语音仅 1 人）"
            } else {
                "（已达视频人数上限）"
            }
        } else {
            ""
        };
        let avatar = tg_avatar(
            c.display_name.chars().take(2).collect::<String>(),
            self.font,
            36.0,
        );
        let mark = if selected { "✓" } else { " " };
        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(avatar)
            .with_child(
                Container::new(
                    ui_text::body(
                        if limit_hint.is_empty() {
                            c.display_name.clone()
                        } else {
                            format!("{} {}", c.display_name, limit_hint)
                        },
                        self.font,
                    )
                        .with_color(if at_limit {
                            theme::muted()
                        } else {
                            theme::text()
                        })
                        .finish(),
                )
                .with_padding_left(10.0)
                .finish(),
            )
            .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
            .with_child(
                ui_text::body(mark.to_string(), self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            );
        let participant_name = c.display_name.clone();
        EventHandler::new(
            Container::new(row.finish())
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .with_background(if selected {
                    theme::accent_cool_bg(18)
                } else {
                    ColorU::new(0, 0, 0, 0)
                })
                .finish(),
        )
        .with_automation_label(participant_name)
        .with_automation_id(format!("chat:calls_participant:{id}"))
        .on_left_mouse_down(move |ctx, _, _| {
            if !at_limit || selected {
                ctx.dispatch_typed_action(CallsPanelAction::ToggleParticipant(id.clone()));
            }
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn settings_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.toggle_row(
            "默认静音麦克风（语音）",
            self.voice_prefs.muted,
            CallsPanelAction::ToggleMicMuted,
        ));
        col.add_child(self.toggle_row(
            "默认关闭摄像头（视频）",
            self.video_prefs.muted,
            CallsPanelAction::ToggleCameraMuted,
        ));
        let mic_hint = if self.voice_prefs.mic_id.is_empty() {
            "麦克风设备：系统默认".to_string()
        } else {
            format!("麦克风设备 ID：{}", self.voice_prefs.mic_id)
        };
        let cam_hint = if self.video_prefs.camera_id.is_empty() {
            "摄像头设备：系统默认".to_string()
        } else {
            format!("摄像头设备 ID：{}", self.video_prefs.camera_id)
        };
        col.add_child(
            Container::new(
                ui_text::chat_preview(
                    format!("{mic_hint}\n{cam_hint}\n来电通知：系统通知栏（桌面已启用）"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_uniform_padding(16.0)
            .finish(),
        );
        col.finish()
    }

    fn toggle_row(&self, label: &str, on: bool, action: CallsPanelAction) -> Box<dyn Element> {
        let row_label = label.to_string();
        EventHandler::new(
            Container::new(
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        Expanded::new(
                            1.0,
                            ui_text::body(row_label.clone(), self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .finish(),
                    )
                    .with_child(
                        ui_text::body(if on { "开" } else { "关" }.to_string(), self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(14.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
        )
        .with_automation_label(row_label)
        .with_automation_id(match &action {
            CallsPanelAction::ToggleMicMuted => "chat:calls_toggle_mic",
            CallsPanelAction::ToggleCameraMuted => "chat:calls_toggle_camera",
            _ => "chat:calls_toggle",
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn privacy_body(&self) -> Box<dyn Element> {
        let rows = [
            ("记录范围", "我的全部通话"),
            ("存储位置", "私有云对象存储（R2）"),
            ("保留时间", "通话结束后 3 天"),
            ("保存内容", "仅通话元数据"),
            ("不含", "音视频与 SDP"),
        ];
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for (k, v) in rows {
            col.add_child(
                Container::new(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_child(
                            Expanded::new(
                                1.0,
                                ui_text::body(k.to_string(), self.font)
                                    .with_color(theme::muted())
                                    .finish(),
                            )
                            .finish(),
                        )
                        .with_child(
                            ui_text::body(v.to_string(), self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .finish(),
                )
                .with_uniform_padding(14.0)
                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                .finish(),
            );
        }
        col.finish()
    }

    fn confirm_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        col.add_child(
            Container::new(
                ui_text::body(
                    "将删除你账号下尚未过期的通话元数据。进行中的通话不会被清除。此操作不可撤销。"
                        .to_string(),
                    self.font,
                )
                .with_color(theme::text())
                .finish(),
            )
            .with_uniform_padding(18.0)
            .finish(),
        );
        col.add_child(Expanded::new(1.0, Flex::column().finish()).finish());
        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        actions.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("取消".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(12.0)
                .finish(),
            )
            .with_automation_label("取消")
            .with_automation_id("chat:calls_clear_cancel")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(CallsPanelAction::BackFromSub);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body(
                        if self.clearing {
                            "清除中…".to_string()
                        } else {
                            "清除我的记录".to_string()
                        },
                        self.font,
                    )
                    .with_color(theme::danger())
                    .finish(),
                )
                .with_uniform_padding(12.0)
                .finish(),
            )
            .with_automation_label(if self.clearing {
                "清除中…"
            } else {
                "清除我的记录"
            })
            .with_automation_id("chat:calls_clear_confirm")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(CallsPanelAction::ConfirmClear);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_padding_right(10.0)
                .with_padding_bottom(12.0)
                .finish(),
        );
        col.finish()
    }

    fn more_menu_overlay(&self) -> Box<dyn Element> {
        let mut menu = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        for (label, action) in [
            ("通话设置", CallsPanelAction::OpenSettings),
            ("通话隐私", CallsPanelAction::OpenPrivacy),
            ("清除我的记录", CallsPanelAction::OpenConfirmClear),
        ] {
            let menu_label = label.to_string();
            let menu_id = match &action {
                CallsPanelAction::OpenSettings => "chat:calls_menu_settings",
                CallsPanelAction::OpenPrivacy => "chat:calls_menu_privacy",
                CallsPanelAction::OpenConfirmClear => "chat:calls_menu_clear",
                _ => "chat:calls_menu_item",
            };
            menu.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body(menu_label.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_uniform_padding(12.0)
                    .finish(),
                )
                .with_automation_label(menu_label)
                .with_automation_id(menu_id)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            );
        }
        let panel = Container::new(
            ConstrainedBox::new(menu.finish())
                .with_width(180.0)
                .finish(),
        )
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
        .finish();
        Align::new(panel)
            .top_right()
            .finish()
    }
}

impl TypedActionView for CallsPanelView {
    type Action = CallsPanelAction;

    fn handle_action(&mut self, action: &CallsPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            CallsPanelAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_calls();
                }
                ctx.emit(CallsPanelEvent::Closed);
                ctx.notify();
            }
            CallsPanelAction::OpenPicker => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.calls_subview = "picker".into();
                    state.calls_menu_open = false;
                }
                self.selected.clear();
                ctx.notify();
            }
            CallsPanelAction::BackOverview | CallsPanelAction::BackFromSub => {
                if let Ok(mut state) = self.shell_state.lock() {
                    let sub = state.calls_subview.as_str();
                    state.calls_subview = match sub {
                        "confirm" | "settings" | "privacy" | "picker" => "overview".into(),
                        _ => "overview".into(),
                    };
                    state.calls_menu_open = false;
                }
                ctx.notify();
            }
            CallsPanelAction::SetCallType(voice) => {
                self.call_type_voice = *voice;
                self.selected.clear();
                ctx.notify();
            }
            CallsPanelAction::ToggleParticipant(id) => {
                if self.selected.contains(id) {
                    self.selected.remove(id);
                } else if self.selected.len() < self.max_others() {
                    self.selected.insert(id.clone());
                }
                ctx.notify();
            }
            CallsPanelAction::PrepareCall => self.prepare_call(ctx),
            CallsPanelAction::ToggleMoreMenu => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.calls_menu_open = !state.calls_menu_open;
                }
                ctx.notify();
            }
            CallsPanelAction::OpenSettings => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.calls_menu_open = false;
                    state.calls_subview = "settings".into();
                }
                ctx.notify();
            }
            CallsPanelAction::OpenPrivacy => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.calls_menu_open = false;
                    state.calls_subview = "privacy".into();
                }
                ctx.notify();
            }
            CallsPanelAction::OpenConfirmClear => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.calls_menu_open = false;
                    state.calls_subview = "confirm".into();
                }
                ctx.notify();
            }
            CallsPanelAction::ConfirmClear => {
                if !self.clearing {
                    self.clear_history(ctx);
                }
            }
            CallsPanelAction::ToggleMicMuted => {
                self.voice_prefs.muted = !self.voice_prefs.muted;
                self.persist_prefs();
                ctx.notify();
            }
            CallsPanelAction::ToggleCameraMuted => {
                self.video_prefs.muted = !self.video_prefs.muted;
                self.persist_prefs();
                ctx.notify();
            }
            CallsPanelAction::Refresh => self.reload(ctx),
            CallsPanelAction::RejoinHistory { call_id } => {
                self.rejoin_history(call_id, ctx);
            }
        }
    }
}

/// Open a local video-chat viewer URL in the system browser.
pub fn open_video_viewer_url(url: &str) -> Result<(), String> {
    open_external_url(url)
}

/// Route for starting video from the calls picker by selected peer count.
///
/// - 1 peer → P2P `chat_video_call_*` (iroh live)
/// - ≥2 peers → SFU `video_chat_*` (Cloudflare Realtime)
pub fn video_call_route_for_selection(selected_count: usize) -> Option<&'static str> {
    match selected_count {
        0 => None,
        1 => Some("p2p"),
        _ => Some("sfu"),
    }
}

#[cfg(test)]
mod video_route_tests {
    use super::video_call_route_for_selection;

    #[test]
    fn one_peer_uses_p2p_not_sfu() {
        assert_eq!(video_call_route_for_selection(1), Some("p2p"));
        assert_ne!(video_call_route_for_selection(1), Some("sfu"));
    }

    #[test]
    fn two_or_more_use_sfu() {
        assert_eq!(video_call_route_for_selection(2), Some("sfu"));
        assert_eq!(video_call_route_for_selection(7), Some("sfu"));
    }
}
