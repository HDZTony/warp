use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::header_menu::{header_button, header_button_colors, header_menu_panel};
use crate::ui::chat::image_asset::{insert_wallpaper_asset, load_wallpaper_bytes_from_path};
use crate::ui::chat::labels::{
    chat_avatar_for_os, conversation_device_title, conversation_os_label, find_cluster_node,
    remote_desktop_peer_identity,
};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::chat::voice_call_ui::{
    accept, apply_voice_status, cancel, cancel_video, decline, end, end_video, fetch_status,
    fetch_video_status, invite, invite_video, record_incoming_ringing, video_error_toast,
    video_status_to_header_line, voice_error_toast, voice_status_to_header_line,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{tg_avatar, StatusTone, TG_HEADER_AVATAR_SIZE};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::call_history::CallHistoryKind;
use wormhole_desktop_core::chat_commands::{
    chat_clear_history, chat_list_conversations, ClearChatHistoryParams,
};
use wormhole_desktop_core::chat_ui_prefs::{
    clear_chat_wallpaper, install_chat_wallpaper_from_path, load_chat_ui_prefs,
    set_chat_muted_until, MUTE_FOREVER,
};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

fn normalized_presence(online: bool, raw: &str) -> &'static str {
    if online || raw == "online" {
        "online"
    } else {
        match raw {
            "signed_in" | "recently_seen" => "recently_online",
            "offline" => "offline",
            _ => "unknown",
        }
    }
}

fn presence_label(presence: &str) -> String {
    match presence {
        "online" => wormhole_i18n::t("chat.presence.online"),
        "recently_online" => wormhole_i18n::t("chat.presence.recently_online"),
        "offline" => wormhole_i18n::t("chat.presence.offline"),
        _ => wormhole_i18n::t("chat.presence.unknown"),
    }
}

fn header_status(voice_phase: &str, presence: &str) -> String {
    if voice_phase == "idle" {
        presence_label(presence).into()
    } else {
        voice_status_to_header_line(voice_phase, presence == "online")
    }
}

pub const TG_HEADER_HEIGHT: f32 = 54.0;
const TG_HEADER_BTN: f32 = 40.0;
const TG_HEADER_MORE_BTN: f32 = 44.0;
const TG_HEADER_PAD_X: f32 = 16.0;
const TG_HEADER_PAD_Y: f32 = 8.0;
const TG_HEADER_INFO_GAP: f32 = 10.0;
const TG_HEADER_ACTION_GAP: f32 = 0.0;

#[derive(Debug, Clone)]
pub enum ChatHeaderAction {
    ToggleThreadSearch,
    OpenRemoteDesktop,
    ToggleProfile,
    ToggleHeaderMenu,
    ToggleMuteFlyout,
    OpenProfile,
    OpenProfileFromInfo,
    MuteFor { seconds: u64 },
    MuteForever,
    Unmute,
    ClearHistory,
    DeleteChat,
    VoiceCallPrimary,
    VideoCallPrimary,
    VoiceCallAccept,
    VoiceCallDecline,
    SetWallpaper,
    ClearWallpaper,
}

#[derive(Debug, Clone)]
pub enum ChatHeaderEvent {
    OpenRemoteDesktop { peer: String },
    OpenLiveViewer { peer: String, title: String },
}

pub struct ChatHeaderView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    title: String,
    status: String,
    online: bool,
    presence: String,
    peer_endpoint: String,
    os: String,
    last_selection: Option<String>,
    last_selection_tick: u64,
    last_wallpaper_tick: u64,
    has_custom_wallpaper: bool,
    last_voice_conv: Option<String>,
    last_voice_message_tick: u64,
    muted: bool,
}

impl ChatHeaderView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            selection,
            shell_state,
            font,
            title: wormhole_i18n::t("chat.header.select_device"),
            status: wormhole_i18n::t("chat.header.select_from_list"),
            online: false,
            presence: "unknown".into(),
            peer_endpoint: String::new(),
            os: String::new(),
            last_selection: None,
            last_selection_tick: 0,
            last_wallpaper_tick: 0,
            has_custom_wallpaper: false,
            last_voice_conv: None,
            last_voice_message_tick: 0,
            muted: false,
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            },
            |view, _, ctx| {
                view.poll(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn poll(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let (selection_tick, pending, selected_summary, wallpaper_tick, message_tick) = self
            .shell_state
            .lock()
            .map(|state| {
                (
                    state.selection_tick,
                    state.pending_open.clone(),
                    state.selected_summary.clone(),
                    state.wallpaper_tick,
                    state.message_tick,
                )
            })
            .unwrap_or((0, None, None, 0, 0));
        let selection_changed = selected != self.last_selection;
        let tick_changed = selection_tick != self.last_selection_tick;
        let wallpaper_changed = wallpaper_tick != self.last_wallpaper_tick;
        if wallpaper_changed {
            self.last_wallpaper_tick = wallpaper_tick;
            self.refresh_wallpaper_flag(ctx);
        }
        if let Some(conv_id) = selected.as_ref() {
            let voice_message_changed = message_tick != self.last_voice_message_tick;
            if selection_changed
                || tick_changed
                || voice_message_changed
                || self.last_voice_conv.as_deref() != Some(conv_id.as_str())
            {
                self.last_voice_message_tick = message_tick;
                self.poll_voice_status(conv_id, ctx);
            }
        } else if self.last_voice_conv.take().is_some() {
            if let Ok(mut state) = self.shell_state.lock() {
                state.set_voice_call_phase("idle");
            }
        }
        if !selection_changed && !tick_changed {
            return;
        }
        self.last_selection = selected.clone();
        self.last_selection_tick = selection_tick;

        if let Some(pending) = pending {
            self.title = pending.title;
            self.status = wormhole_i18n::t("chat.header.opening");
            self.presence = pending.presence;
            self.online = self.presence == "online";
            self.os = pending.os;
            self.peer_endpoint.clear();
            ctx.notify();
            if selected.is_some() {
                // Still resolve once conv_id lands.
                self.refresh_from_selection(ctx);
            }
            return;
        }

        if selected.is_none() {
            self.title = wormhole_i18n::t("chat.header.select_device");
            self.status = wormhole_i18n::t("chat.header.select_from_list");
            self.online = false;
            self.presence = "unknown".into();
            self.peer_endpoint.clear();
            self.os.clear();
            ctx.notify();
            return;
        }
        if let Some(summary) = selected_summary {
            self.title = summary.title;
            self.os = summary.os;
            self.presence = summary.presence;
            self.online = self.presence == "online";
            self.status = presence_label(&self.presence).into();
            ctx.notify();
        }
        self.refresh_from_selection(ctx);
    }

    pub(crate) fn selection_changed(&mut self, ctx: &mut ViewContext<Self>) {
        self.poll(ctx);
    }

    fn refresh_wallpaper_flag(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(conv_id) = conv_id else {
            self.has_custom_wallpaper = false;
            ctx.notify();
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let data_dir = core.data_dir();
                let prefs = load_chat_ui_prefs(&data_dir).await.unwrap_or_default();
                prefs.wallpaper_path(&conv_id).is_some()
            },
            |view, has_wallpaper, ctx| {
                view.has_custom_wallpaper = has_wallpaper;
                ctx.notify();
            },
        );
    }

    pub(crate) fn poll_voice_status(&mut self, conv_id: &str, ctx: &mut ViewContext<Self>) {
        self.last_voice_conv = Some(conv_id.to_string());
        let core = self.core.clone();
        let conv_id = conv_id.to_string();
        let shell_state = self.shell_state.clone();
        ctx.spawn(
            async move { fetch_status(&core, &conv_id).await },
            move |view, output, ctx| {
                match output {
                    Ok(status) => {
                        apply_voice_status(&shell_state, &status);
                        view.status = voice_status_to_header_line(&status.phase, view.online);
                        if status.phase == "incoming" {
                            let core = view.core.clone();
                            let conv = view.last_voice_conv.clone().unwrap_or_default();
                            let call_id = status.call_id.clone();
                            if !conv.is_empty() {
                                ctx.spawn(
                                    async move {
                                        record_incoming_ringing(
                                            &core,
                                            &conv,
                                            call_id.as_deref(),
                                            CallHistoryKind::Voice,
                                        )
                                        .await;
                                    },
                                    |_view, _, _ctx| {},
                                );
                            }
                        }
                        if status.phase == "active" && status.peer_live {
                            let peer = view.peer_endpoint.trim().to_string();
                            let should_open = view
                                .shell_state
                                .lock()
                                .map(|state| {
                                    state.voice_live_peer.as_deref() != Some(peer.as_str())
                                })
                                .unwrap_or(true);
                            if !peer.is_empty() && should_open {
                                if let Ok(mut state) = view.shell_state.lock() {
                                    state.voice_live_peer = Some(peer.clone());
                                }
                                let title = format!("语音 · {}", view.title);
                                ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                            }
                        }
                    }
                    Err(_) => {}
                }
                ctx.notify();
            },
        );
    }

    fn selected_conv_id(&self) -> Option<String> {
        self.selection.lock().ok().and_then(|g| g.clone())
    }

    pub(crate) fn trigger_voice_call(&mut self, ctx: &mut ViewContext<Self>) {
        self.open_or_toggle_outgoing(ctx, false);
    }

    pub(crate) fn trigger_video_call(&mut self, ctx: &mut ViewContext<Self>) {
        self.open_or_toggle_outgoing(ctx, true);
    }

    /// Open Telegram-style confirm panel when idle; otherwise cancel/end like before.
    fn open_or_toggle_outgoing(&mut self, ctx: &mut ViewContext<Self>, prefer_video: bool) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(
                        wormhole_i18n::t("chat.toast.select_conversation"),
                        StatusTone::Muted,
                    );
                }
                ctx.notify();
                return;
            }
        };
        let phase = self
            .shell_state
            .lock()
            .map(|state| state.voice_call_phase.clone())
            .unwrap_or_else(|_| "idle".into());
        match phase.as_str() {
            "idle" => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_overlays();
                    state.open_outgoing_call_confirm(
                        conv_id,
                        self.title.clone(),
                        self.os.clone(),
                    );
                }
                let _ = prefer_video;
                ctx.notify();
            }
            "ringing" => {
                if prefer_video {
                    self.spawn_video_primary(ctx);
                } else {
                    self.spawn_voice_primary(ctx);
                }
            }
            "active" => {
                if prefer_video {
                    self.spawn_video_primary(ctx);
                } else {
                    self.spawn_voice_primary(ctx);
                }
            }
            _ => {
                // incoming: keep existing accept/decline on banner; ignore primary
                ctx.notify();
            }
        }
    }

    pub(crate) fn confirm_outgoing_voice(&mut self, ctx: &mut ViewContext<Self>) {
        self.spawn_invite_from_panel(ctx, false);
    }

    pub(crate) fn confirm_outgoing_video(&mut self, ctx: &mut ViewContext<Self>) {
        self.spawn_invite_from_panel(ctx, true);
    }

    pub(crate) fn cancel_outgoing_panel(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selected_conv_id();
        let (phase, video, conv_id) = self
            .shell_state
            .lock()
            .map(|state| {
                let video = state
                    .outgoing_call_ui
                    .as_ref()
                    .is_some_and(|ui| ui.is_video_ringing());
                let conv = state
                    .outgoing_call_ui
                    .as_ref()
                    .map(|ui| ui.conv_id().to_string())
                    .or(selected);
                (state.voice_call_phase.clone(), video, conv)
            })
            .unwrap_or_else(|_| ("idle".into(), false, None));

        if phase == "ringing" {
            if let Some(conv_id) = conv_id {
                let core = self.core.clone();
                let shell_state = self.shell_state.clone();
                let future = async move {
                    if video {
                        cancel_video(&core, &conv_id).await
                    } else {
                        cancel(&core, &conv_id).await
                    }
                };
                ctx.spawn(future, move |view, output, ctx| {
                    match output {
                        Ok(status) => {
                            apply_voice_status(&shell_state, &status);
                            view.status = if video {
                                video_status_to_header_line(&status.phase, view.online)
                            } else {
                                voice_status_to_header_line(&status.phase, view.online)
                            };
                        }
                        Err(err) => {
                            let (text, tone) = if video {
                                video_error_toast(&err)
                            } else {
                                voice_error_toast(&err)
                            };
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(text, tone);
                                state.clear_outgoing_call_ui();
                            }
                        }
                    }
                    ctx.notify();
                });
            }
        } else if let Ok(mut state) = self.shell_state.lock() {
            state.clear_outgoing_call_ui();
        }
        ctx.notify();
    }

    fn spawn_invite_from_panel(&mut self, ctx: &mut ViewContext<Self>, video: bool) {
        let conv_id = self
            .shell_state
            .lock()
            .ok()
            .and_then(|state| {
                state
                    .outgoing_call_ui
                    .as_ref()
                    .map(|ui| ui.conv_id().to_string())
            })
            .or_else(|| self.selected_conv_id());
        let Some(conv_id) = conv_id else {
            return;
        };
        // Telegram-style: switch to ringing UI immediately while invite is in flight.
        // Presence may be stale; let core invite fail explicitly rather than blocking the panel.
        if let Ok(mut state) = self.shell_state.lock() {
            state.set_outgoing_call_ringing(video);
            state.set_voice_call_phase("ringing");
        }
        self.status = if video {
            video_status_to_header_line("ringing", self.online)
        } else {
            voice_status_to_header_line("ringing", self.online)
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let request_conv_id = conv_id.clone();
        let future = async move {
            if video {
                invite_video(&core, &request_conv_id).await
            } else {
                invite(&core, &request_conv_id).await
            }
        };
        ctx.spawn(future, move |view, output, ctx| match output {
            Ok(status) => {
                apply_voice_status(&shell_state, &status);
                if status.phase == "ringing" {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.set_outgoing_call_ringing(video);
                    }
                }
                view.status = if video {
                    video_status_to_header_line(&status.phase, view.online)
                } else {
                    voice_status_to_header_line(&status.phase, view.online)
                };
                if status.phase == "active" && status.peer_live {
                    let peer = view.peer_endpoint.trim().to_string();
                    if !peer.is_empty() {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.voice_live_peer = Some(peer.clone());
                        }
                        let title = if video {
                            format!("视频 · {}", view.title)
                        } else {
                            format!("语音 · {}", view.title)
                        };
                        ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                    }
                }
                if video {
                    view.poll_video_status(&conv_id, ctx);
                } else {
                    view.poll_voice_status(&conv_id, ctx);
                }
                ctx.notify();
            }
            Err(err) => {
                let (text, tone) = if video {
                    video_error_toast(&err)
                } else {
                    voice_error_toast(&err)
                };
                if let Ok(mut state) = view.shell_state.lock() {
                    state.show_toast(text, tone);
                    state.set_voice_call_phase("idle");
                    // Keep Ringing overlay so the user can cancel; invite already failed.
                    if state.outgoing_call_ui.is_none() {
                        // Defensive: if cleared elsewhere, reopen confirming.
                        state.open_outgoing_call_confirm(
                            &conv_id,
                            view.title.clone(),
                            view.os.clone(),
                        );
                    } else {
                        state.bump_overlay_tick();
                    }
                }
                view.status = voice_status_to_header_line("idle", view.online);
                ctx.notify();
            }
        });
        ctx.notify();
    }

    fn spawn_video_primary(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.select_conversation"), StatusTone::Muted);
                }
                ctx.notify();
                return;
            }
        };
        let phase = self
            .shell_state
            .lock()
            .map(|state| state.voice_call_phase.clone())
            .unwrap_or_else(|_| "idle".into());
        if phase == "idle" && !self.online {
            if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.peer_offline_video"), StatusTone::Muted);
            }
            ctx.notify();
            return;
        }
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let request_conv_id = conv_id.clone();
        let future = async move {
            match phase.as_str() {
                "ringing" => cancel_video(&core, &request_conv_id).await,
                "active" => end_video(&core, &request_conv_id).await,
                _ => invite_video(&core, &request_conv_id).await,
            }
        };
        ctx.spawn(future, move |view, output, ctx| match output {
            Ok(status) => {
                apply_voice_status(&shell_state, &status);
                view.status = video_status_to_header_line(&status.phase, view.online);
                if status.phase == "ringing" {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.video_ringing"), StatusTone::Neutral);
                    }
                } else if status.phase == "active" && status.peer_live {
                    let peer = view.peer_endpoint.trim().to_string();
                    if !peer.is_empty() {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.voice_live_peer = Some(peer.clone());
                        }
                        let title = format!("视频 · {}", view.title);
                        ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                    }
                } else if status.phase == "idle" {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.call_ended"), StatusTone::Muted);
                        state.voice_live_peer = None;
                    }
                }
                view.poll_video_status(&conv_id, ctx);
            }
            Err(err) => {
                let (text, tone) = video_error_toast(&err);
                if let Ok(mut state) = view.shell_state.lock() {
                    state.show_toast(text, tone);
                }
            }
        });
        ctx.notify();
    }

    pub(crate) fn poll_video_status(&mut self, conv_id: &str, ctx: &mut ViewContext<Self>) {
        self.last_voice_conv = Some(conv_id.to_string());
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let conv = conv_id.to_string();
        ctx.spawn(
            async move { fetch_video_status(&core, &conv).await },
            move |view, output, ctx| {
                if let Ok(status) = output {
                    apply_voice_status(&shell_state, &status);
                    view.status = video_status_to_header_line(&status.phase, view.online);
                    if status.phase == "incoming" {
                        let core = view.core.clone();
                        let conv = view.last_voice_conv.clone().unwrap_or_default();
                        let call_id = status.call_id.clone();
                        if !conv.is_empty() {
                            ctx.spawn(
                                async move {
                                    record_incoming_ringing(
                                        &core,
                                        &conv,
                                        call_id.as_deref(),
                                        CallHistoryKind::Video,
                                    )
                                    .await;
                                },
                                |_view, _, _ctx| {},
                            );
                        }
                    }
                    if status.phase == "active" && status.peer_live {
                        let peer = view.peer_endpoint.trim().to_string();
                        if !peer.is_empty() {
                            let already = view
                                .shell_state
                                .lock()
                                .ok()
                                .and_then(|s| s.voice_live_peer.clone());
                            if already.as_deref() != Some(peer.as_str()) {
                                if let Ok(mut state) = view.shell_state.lock() {
                                    state.voice_live_peer = Some(peer.clone());
                                }
                                let title = format!("视频 · {}", view.title);
                                ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                            }
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn spawn_voice_primary(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.select_conversation"), StatusTone::Muted);
                }
                ctx.notify();
                return;
            }
        };
        let phase = self
            .shell_state
            .lock()
            .map(|state| state.voice_call_phase.clone())
            .unwrap_or_else(|_| "idle".into());
        if phase == "idle" && !self.online {
            if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.peer_offline_voice"), StatusTone::Muted);
            }
            ctx.notify();
            return;
        }
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let request_conv_id = conv_id.clone();
        let future = async move {
            match phase.as_str() {
                "ringing" => cancel(&core, &request_conv_id).await,
                "active" => end(&core, &request_conv_id).await,
                _ => invite(&core, &request_conv_id).await,
            }
        };
        ctx.spawn(future, move |view, output, ctx| match output {
            Ok(status) => {
                apply_voice_status(&shell_state, &status);
                view.status = voice_status_to_header_line(&status.phase, view.online);
                if status.phase == "ringing" {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.call_ringing"), StatusTone::Neutral);
                    }
                } else if status.phase == "active" && status.peer_live {
                    let peer = view.peer_endpoint.trim().to_string();
                    if !peer.is_empty() {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.voice_live_peer = Some(peer.clone());
                        }
                        let title = format!("语音 · {}", view.title);
                        ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                    }
                } else if status.phase == "idle" {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.call_ended"), StatusTone::Muted);
                        state.voice_live_peer = None;
                    }
                }
                view.poll_voice_status(&conv_id, ctx);
            }
            Err(err) => {
                let (text, tone) = voice_error_toast(&err);
                if let Ok(mut state) = view.shell_state.lock() {
                    state.show_toast(text, tone);
                }
            }
        });
        ctx.notify();
    }

    fn spawn_voice_accept(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => return,
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let request_conv_id = conv_id.clone();
        ctx.spawn(
            async move { accept(&core, &request_conv_id).await },
            move |view, output, ctx| match output {
                Ok(status) => {
                    apply_voice_status(&shell_state, &status);
                    view.status = voice_status_to_header_line(&status.phase, view.online);
                    if status.phase == "active" && status.peer_live {
                        let peer = view.peer_endpoint.trim().to_string();
                        if !peer.is_empty() {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.voice_live_peer = Some(peer.clone());
                            }
                            let title = format!("语音 · {}", view.title);
                            ctx.emit(ChatHeaderEvent::OpenLiveViewer { peer, title });
                        }
                    }
                    view.poll_voice_status(&conv_id, ctx);
                }
                Err(err) => {
                    let (text, tone) = voice_error_toast(&err);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(text, tone);
                    }
                }
            },
        );
        ctx.notify();
    }

    fn spawn_voice_decline(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => return,
        };
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        let request_conv_id = conv_id.clone();
        ctx.spawn(
            async move { decline(&core, &request_conv_id).await },
            move |view, output, ctx| match output {
                Ok(status) => {
                    apply_voice_status(&shell_state, &status);
                    view.status = voice_status_to_header_line(&status.phase, view.online);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.call_declined"), StatusTone::Muted);
                    }
                    view.poll_voice_status(&conv_id, ctx);
                }
                Err(err) => {
                    let (text, tone) = voice_error_toast(&err);
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(text, tone);
                    }
                }
            },
        );
        ctx.notify();
    }

    fn pick_and_install_wallpaper(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.select_conversation"), StatusTone::Muted);
                }
                ctx.notify();
                return;
            }
        };
        if let Ok(mut state) = self.shell_state.lock() {
            state.header_menu_open = false;
        }
        let core = self.core.clone();
        let shell_state = self.shell_state.clone();
        ctx.spawn(
            async move {
                let picked = tokio::task::spawn_blocking(|| {
                    rfd::FileDialog::new()
                        .set_title(wormhole_i18n::t("chat.wallpaper.picker_title"))
                        .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                        .pick_file()
                })
                .await
                .ok()
                .flatten();
                let Some(path) = picked else {
                    return Ok::<_, String>(None);
                };
                let data_dir = core.data_dir();
                let prefs =
                    install_chat_wallpaper_from_path(&data_dir, &conv_id, path.as_path()).await?;
                let rel = prefs
                    .wallpaper_path(&conv_id)
                    .ok_or_else(|| wormhole_i18n::t("chat.wallpaper.path_missing"))?;
                let abs = wormhole_desktop_core::chat_wallpaper_storage::wallpaper_abs_path(
                    &data_dir, rel,
                );
                let bytes = load_wallpaper_bytes_from_path(&abs)?;
                Ok(Some((conv_id, bytes)))
            },
            |view, output, ctx| match output {
                Ok(Some((conv_id, bytes))) => {
                    if insert_wallpaper_asset(ctx, &conv_id, bytes).is_ok() {
                        view.has_custom_wallpaper = true;
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(wormhole_i18n::t("chat.wallpaper.updated"), StatusTone::Success);
                            state.bump_wallpaper_tick();
                            state.bump_prefs_tick();
                        }
                    }
                    ctx.notify();
                }
                Ok(None) => {}
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.wallpaper.set_failed",
                                &[("err", &err.to_string())],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
        ctx.notify();
    }

    fn clear_wallpaper(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selected_conv_id() {
            Some(id) => id,
            None => return,
        };
        if let Ok(mut state) = self.shell_state.lock() {
            state.header_menu_open = false;
        }
        let core = self.core.clone();
        ctx.spawn(
            async move {
                clear_chat_wallpaper(&core.data_dir(), &conv_id)
                    .await
                    .map(|_| conv_id)
            },
            |view, output, ctx| match output {
                Ok(_) => {
                    view.has_custom_wallpaper = false;
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.wallpaper.restored"), StatusTone::Muted);
                        state.bump_wallpaper_tick();
                        state.bump_prefs_tick();
                    }
                    ctx.notify();
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.wallpaper.clear_failed",
                                &[("err", &err.to_string())],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
        ctx.notify();
    }

    fn refresh_from_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(selected) = selected else {
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let conv_id = selected.clone();
                let conversations = chat_list_conversations(app, &state).await;
                let cluster = cluster_status_hud(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let prefs = load_chat_ui_prefs(&state.data_dir).await.unwrap_or_default();
                let muted = prefs.is_muted(&conv_id);
                (conv_id, conversations, cluster, remarks, muted)
            },
            |view, output, ctx| {
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                let (conv_id, conversations, cluster, remarks, muted) = output;
                if conv_id != selected {
                    return;
                }
                view.muted = muted;
                if view
                    .shell_state
                    .lock()
                    .map(|state| state.pending_open.is_some())
                    .unwrap_or(false)
                {
                    return;
                }
                let voice_phase = view
                    .shell_state
                    .lock()
                    .map(|state| state.voice_call_phase.clone())
                    .unwrap_or_else(|_| "idle".into());
                let conv = conversations
                    .ok()
                    .and_then(|list| list.into_iter().find(|conv| conv.id == selected));
                if let Some(conv) = conv {
                    let cluster_ref = cluster.as_ref().ok();
                    let remark = find_cluster_node(&conv, cluster_ref)
                        .and_then(|node| remarks.get(&node.node_id).map(String::as_str));
                    view.title = display_name_with_remark(remark, || {
                        conversation_device_title(&conv, cluster_ref)
                    });
                    view.os = conversation_os_label(&conv, cluster_ref);
                    view.peer_endpoint = remote_desktop_peer_identity(
                        Some(&conv),
                        find_cluster_node(&conv, cluster_ref),
                    )
                    .unwrap_or_default();
                    let peer_node = find_cluster_node(&conv, cluster_ref);
                    let peer_online = peer_node.map(|node| node.online).unwrap_or(false);
                    view.presence = peer_node
                        .map(|node| normalized_presence(node.online, &node.presence_status))
                        .unwrap_or("unknown")
                        .into();
                    view.online = peer_online;
                    view.status = header_status(&voice_phase, &view.presence);
                } else if let Ok(cluster) = cluster {
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(selected.as_str())
                            || n.node_id == selected
                    }) {
                        view.title = display_name_with_remark(
                            remarks.get(&node.node_id).map(String::as_str),
                            || format!("{} · {}", node.os, node.hostname),
                        );
                        view.online = node.online;
                        view.presence =
                            normalized_presence(node.online, &node.presence_status).into();
                        view.peer_endpoint =
                            remote_desktop_peer_identity(None, Some(node)).unwrap_or_default();
                        view.os = node.os.clone();
                        view.status = header_status(&voice_phase, &view.presence);
                    } else {
                        view.title = selected.clone();
                        view.peer_endpoint.clear();
                        view.os.clear();
                        view.online = false;
                        view.presence = "unknown".into();
                        view.status = header_status(&voice_phase, &view.presence);
                    }
                } else {
                    view.title = selected.clone();
                    view.peer_endpoint.clear();
                    view.os.clear();
                    view.online = false;
                    view.presence = "unknown".into();
                    view.status = header_status(&voice_phase, &view.presence);
                }
                ctx.notify();
            },
        );
    }

    fn avatar_label(&self) -> String {
        if self.title == wormhole_i18n::t("chat.header.select_device") {
            return "WH".to_string();
        }
        if !self.os.is_empty() {
            return chat_avatar_for_os(&self.os);
        }
        chat_avatar_for_os("")
    }

    pub fn live_peer_node_id(&self) -> String {
        self.peer_endpoint.trim().to_string()
    }

    pub fn live_viewer_title(&self) -> String {
        format!("语音 · {}", self.title)
    }

    fn spawn_set_mute(&mut self, until: Option<u64>, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        if let Ok(mut state) = self.shell_state.lock() {
            state.header_menu_open = false;
            state.mute_flyout_open = false;
        }
        let Some(conv_id) = selected else {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(
                    wormhole_i18n::t("chat.toast.select_conversation"),
                    StatusTone::Muted,
                );
            }
            ctx.notify();
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                set_chat_muted_until(&state.data_dir, &conv_id, until).await
            },
            move |view, output, ctx| match output {
                Ok(_) => {
                    view.muted = until.is_some();
                    if let Ok(mut state) = view.shell_state.lock() {
                        let key = if until.is_some() {
                            "chat.toast.mute_on"
                        } else {
                            "chat.toast.mute_off"
                        };
                        state.show_toast(wormhole_i18n::t(key), StatusTone::Success);
                        state.bump_prefs_tick();
                    }
                    ctx.notify();
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.toast.mute_update_failed",
                                &[("err", &err.to_string())],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
        ctx.notify();
    }

    fn spawn_clear_history(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        if let Ok(mut state) = self.shell_state.lock() {
            state.header_menu_open = false;
            state.mute_flyout_open = false;
        }
        let Some(conv_id) = selected else {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(
                    wormhole_i18n::t("chat.toast.select_conversation"),
                    StatusTone::Muted,
                );
            }
            ctx.notify();
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                chat_clear_history(&state, ClearChatHistoryParams { conv_id }).await
            },
            |view, output, ctx| match output {
                Ok(conv) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t("chat.history_cleared"),
                            StatusTone::Success,
                        );
                        state.mark_history_cleared(&conv.id);
                    }
                    ctx.notify();
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.toast.clear_history_failed",
                                &[("err", &err)],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
        ctx.notify();
    }

    fn shell_flags(&self) -> (bool, bool, bool) {
        self.shell_state
            .lock()
            .map(|state| {
                (
                    state.thread_search_open,
                    state.profile_open,
                    state.header_menu_open,
                )
            })
            .unwrap_or((false, false, false))
    }
}

impl Entity for ChatHeaderView {
    type Event = ChatHeaderEvent;
}

impl View for ChatHeaderView {
    fn ui_name() -> &'static str {
        "ChatHeaderView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let (search_open, profile_open, menu_open) = self.shell_flags();
        let (voice_phase, voice_active) = self
            .shell_state
            .lock()
            .map(|state| (state.voice_call_phase.clone(), state.voice_call_active))
            .unwrap_or_else(|_| ("idle".into(), false));
        let phone_active =
            voice_active || matches!(voice_phase.as_str(), "ringing" | "incoming" | "active");
        let (_, phone_icon_color) = header_button_colors(phone_active, false);
        let mute_flyout_open = self
            .shell_state
            .lock()
            .map(|state| state.mute_flyout_open)
            .unwrap_or(false);
        let muted = self.muted;

        let info_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min)
            .with_child(tg_avatar(self.avatar_label(), self.font, TG_HEADER_AVATAR_SIZE))
            .with_child(
                Container::new({
                    let mut text_col = Flex::column().with_main_axis_size(MainAxisSize::Min);
                    text_col.add_child(
                        ui_text::chat_header_title(self.title.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    );
                    let status_color = if self.online {
                        theme::accent_cool()
                    } else {
                        theme::muted()
                    };
                    text_col.add_child(
                        Container::new(
                            ui_text::chat_header_status(self.status.clone(), self.font)
                                .with_color(status_color)
                                .finish(),
                        )
                        .with_margin_top(1.0)
                        .finish(),
                    );
                    text_col.finish()
                })
                .with_margin_left(TG_HEADER_INFO_GAP)
                .finish(),
            )
            .finish();

        let info_clickable = EventHandler::new(
            ConstrainedBox::new(info_row)
                .with_min_height(TG_HEADER_AVATAR_SIZE)
                .finish(),
        )
        .with_automation_label("聊天信息")
        .with_automation_id("chat:header_info")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatHeaderAction::OpenProfileFromInfo);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Min);
        let buttons = [
            (
                "chat-header-search.svg",
                search_open,
                ChatHeaderAction::ToggleThreadSearch,
                TG_HEADER_BTN,
            ),
            (
                "chat-header-phone.svg",
                phone_active,
                ChatHeaderAction::VoiceCallPrimary,
                TG_HEADER_BTN,
            ),
            (
                "chat-header-info.svg",
                profile_open,
                ChatHeaderAction::ToggleProfile,
                TG_HEADER_BTN,
            ),
            (
                "chat-header-more.svg",
                menu_open,
                ChatHeaderAction::ToggleHeaderMenu,
                TG_HEADER_MORE_BTN,
            ),
        ];
        for (index, (icon, active, action, btn_size)) in buttons.into_iter().enumerate() {
            actions.add_child(
                Container::new(
                    ConstrainedBox::new(header_button(
                        icon,
                        if icon == "chat-header-phone.svg" {
                            phone_icon_color
                        } else {
                            theme::muted()
                        },
                        active,
                        action,
                        btn_size,
                    ))
                    .with_width(btn_size)
                    .with_height(btn_size)
                    .finish(),
                )
                .with_margin_left(if index == 0 {
                    0.0
                } else {
                    TG_HEADER_ACTION_GAP
                })
                .finish(),
            );
        }

        let header_bar = Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(info_clickable)
                            .with_uniform_padding(4.0)
                            .finish(),
                    )
                    .finish(),
                )
                .with_child(actions.finish())
                .finish(),
        )
        .with_padding_left(TG_HEADER_PAD_X)
        .with_padding_right(TG_HEADER_PAD_X)
        .with_padding_top(TG_HEADER_PAD_Y)
        .with_padding_bottom(TG_HEADER_PAD_Y)
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .with_background(theme::panel())
        .finish();

        if !menu_open {
            return header_bar;
        }

        let mut stack = Stack::new();
        stack.add_child(header_bar);
        stack.add_child(
            Align::new(
                Container::new(header_menu_panel(
                    self.font,
                    mute_flyout_open,
                    self.has_custom_wallpaper,
                    muted,
                    profile_open,
                ))
                .with_margin_top(TG_HEADER_HEIGHT + 6.0)
                .with_margin_right(TG_HEADER_PAD_X)
                .finish(),
            )
            .top_right()
            .finish(),
        );
        stack.finish()
    }
}

impl TypedActionView for ChatHeaderView {
    type Action = ChatHeaderAction;

    fn handle_action(&mut self, action: &ChatHeaderAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatHeaderAction::ToggleThreadSearch => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.thread_search_open = !state.thread_search_open;
                    if !state.thread_search_open {
                        state.thread_search_query.clear();
                    }
                    state.close_overlays();
                }
                ctx.notify();
            }
            ChatHeaderAction::OpenRemoteDesktop => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_overlays();
                }
                let peer = self.peer_endpoint.trim().to_string();
                if peer.is_empty() {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.no_rdp_peer"), StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                }
                if !self.online {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.peer_offline_rdp"), StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                }
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(wormhole_i18n::t("chat.toast.opening_rdp"), StatusTone::Neutral);
                }
                ctx.emit(ChatHeaderEvent::OpenRemoteDesktop { peer });
                self.refresh_from_selection(ctx);
            }
            ChatHeaderAction::ToggleProfile => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.profile_open = !state.profile_open;
                    state.close_overlays();
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            ChatHeaderAction::OpenProfile | ChatHeaderAction::OpenProfileFromInfo => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.profile_open = true;
                    state.header_menu_open = false;
                    state.mute_flyout_open = false;
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            ChatHeaderAction::ToggleHeaderMenu => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.header_menu_open = !state.header_menu_open;
                    if !state.header_menu_open {
                        state.mute_flyout_open = false;
                    }
                }
                ctx.notify();
            }
            ChatHeaderAction::ToggleMuteFlyout => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.mute_flyout_open = !state.mute_flyout_open;
                }
                ctx.notify();
            }
            ChatHeaderAction::MuteFor { seconds } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                self.spawn_set_mute(Some(now.saturating_add(*seconds)), ctx);
            }
            ChatHeaderAction::MuteForever => {
                self.spawn_set_mute(Some(MUTE_FOREVER), ctx);
            }
            ChatHeaderAction::Unmute => {
                self.spawn_set_mute(None, ctx);
            }
            ChatHeaderAction::ClearHistory => {
                self.spawn_clear_history(ctx);
            }
            ChatHeaderAction::DeleteChat => {
                let selected = self.selection.lock().ok().and_then(|g| g.clone());
                if let Ok(mut state) = self.shell_state.lock() {
                    state.header_menu_open = false;
                }
                let Some(conv_id) = selected else {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(wormhole_i18n::t("chat.toast.select_conversation"), StatusTone::Muted);
                    }
                    ctx.notify();
                    return;
                };
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        let state = runtime.state.clone();
                        wormhole_desktop_core::chat_ui_prefs::set_chat_hidden(
                            &state.data_dir,
                            &conv_id,
                            true,
                        )
                        .await
                    },
                    |view, output, ctx| match output {
                        Ok(_) => {
                            if let Ok(mut guard) = view.selection.lock() {
                                *guard = None;
                            }
                            if let Ok(mut state) = view.shell_state.lock() {
                                state.show_toast(wormhole_i18n::t("chat.toast.conversation_deleted"), StatusTone::Muted);
                                state.clear_pending_open();
                                state.bump_selection_tick();
                                state.bump_prefs_tick();
                            }
                            view.title = "选择左侧终端".into();
                            view.status = "从列表中选择会话".into();
                            view.online = false;
                            view.peer_endpoint.clear();
                            view.os.clear();
                            ctx.notify();
                        }
                        Err(err) => {
                            if let Ok(mut state) = view.shell_state.lock() {
                                state
                                    .show_toast(
                                        wormhole_i18n::t_args(
                                            "chat.toast.delete_failed",
                                            &[("err", &err.to_string())],
                                        ),
                                        StatusTone::Danger,
                                    );
                            }
                            ctx.notify();
                        }
                    },
                );
                ctx.notify();
            }
            ChatHeaderAction::VoiceCallPrimary => {
                self.trigger_voice_call(ctx);
            }
            ChatHeaderAction::VideoCallPrimary => {
                self.trigger_video_call(ctx);
            }
            ChatHeaderAction::VoiceCallAccept => {
                self.spawn_voice_accept(ctx);
            }
            ChatHeaderAction::VoiceCallDecline => {
                self.spawn_voice_decline(ctx);
            }
            ChatHeaderAction::SetWallpaper => {
                self.pick_and_install_wallpaper(ctx);
            }
            ChatHeaderAction::ClearWallpaper => {
                self.clear_wallpaper(ctx);
            }
        }
    }
}
