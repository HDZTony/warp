use std::collections::BTreeMap;

use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Flex, MainAxisSize, ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::labels::{chat_avatar_for_os, remote_desktop_peer_identity};
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::clipboard::write_clipboard_text;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{tg_avatar, ui_title, StatusTone, TG_AVATAR_LG_SIZE};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_set_peer_display_name, SetChatPeerDisplayNameParams,
};
use wormhole_desktop_core::chat_contacts::{
    chat_contacts_add, chat_contacts_list_manual, contact_matches_peer, ChatContactAddParams,
};
use wormhole_desktop_core::chat_ui_prefs::{
    load_chat_ui_prefs, set_chat_hidden, set_chat_muted_until, MUTE_FOREVER,
};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{
    display_name_with_remark, load_device_remarks, set_device_remark,
};

const MAX_REMARK_CHARS: usize = 40;

#[derive(Debug, Clone)]
pub enum ChatProfileAction {
    Close,
    Message,
    Mute,
    RemoteDesktop,
    CopyUsername,
    AddContact,
    Block,
    FocusRemark,
    RemarkEdit(TextFieldEditAction),
}

#[derive(Debug, Clone)]
pub enum ChatProfileEvent {
    OpenRemoteDesktop { peer: String },
    FocusCompose,
}

pub struct ChatProfilePanelView {
    core: CoreHandle,
    selection: ConversationSelection,
    shell_state: SharedChatShellState,
    font: FamilyId,
    title: String,
    node_id: String,
    peer_endpoint: String,
    peer_user_id: String,
    conv_id: Option<String>,
    status: String,
    online: bool,
    last_seen: u64,
    muted: bool,
    is_contact: bool,
    cluster_name: String,
    os_label: String,
    default_title: String,
    remark: String,
    remark_field: TextFieldState,
    remark_focused: bool,
    remark_dirty: bool,
    caret_blink: CaretBlink,
    refresh_in_flight: bool,
}

impl ChatProfilePanelView {
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
            title: String::new(),
            node_id: String::new(),
            peer_endpoint: String::new(),
            peer_user_id: String::new(),
            conv_id: None,
            status: String::new(),
            online: false,
            last_seen: 0,
            muted: false,
            is_contact: false,
            cluster_name: String::new(),
            os_label: String::new(),
            default_title: String::new(),
            remark: String::new(),
            remark_field: TextFieldState::new(),
            remark_focused: false,
            remark_dirty: false,
            caret_blink: CaretBlink::new(),
            refresh_in_flight: false,
        };
        view.start_poll(ctx);
        view
    }

    fn username_value(&self) -> String {
        if !self.peer_user_id.trim().is_empty() {
            self.peer_user_id.clone()
        } else {
            self.node_id.clone()
        }
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            },
            |view, _, ctx| {
                view.refresh(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        let Some(selected) = selected else {
            self.title.clear();
            self.node_id.clear();
            self.peer_endpoint.clear();
            self.peer_user_id.clear();
            self.conv_id = None;
            self.remark.clear();
            ctx.notify();
            return;
        };
        if self.remark_focused || self.remark_dirty {
            return;
        }
        if self.refresh_in_flight {
            return;
        }
        self.refresh_in_flight = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let state = runtime.state.clone();
                let conversations =
                    wormhole_desktop_core::chat_commands::chat_list_conversations(app, &state)
                        .await;
                let cluster = cluster_status_hud(&state).await;
                let remarks = load_device_remarks(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let prefs = load_chat_ui_prefs(&state.data_dir).await.unwrap_or_default();
                let contacts = chat_contacts_list_manual(&state.data_dir)
                    .await
                    .unwrap_or_default();
                (conversations, cluster, remarks, prefs, contacts)
            },
            |view, output, ctx| {
                view.refresh_in_flight = false;
                if view.remark_focused || view.remark_dirty {
                    ctx.notify();
                    return;
                }
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                let (conversations, cluster, remarks, prefs, contacts) = output;
                view.muted = prefs.is_muted(&selected);
                let conv = conversations
                    .ok()
                    .and_then(|list| list.into_iter().find(|conv| conv.id == selected));
                view.peer_endpoint =
                    remote_desktop_peer_identity(conv.as_ref(), None).unwrap_or_default();
                view.node_id.clear();
                view.peer_user_id = conv
                    .as_ref()
                    .and_then(|c| c.peer_user_id.clone())
                    .unwrap_or_default();
                view.online = false;
                view.last_seen = 0;
                if let Some(ref conv) = conv {
                    view.conv_id = Some(conv.id.clone());
                } else {
                    view.conv_id = Some(selected.clone());
                }
                if let Ok(cluster) = cluster {
                    view.cluster_name = cluster
                        .clusters
                        .iter()
                        .find(|c| c.active)
                        .and_then(|c| c.name.clone())
                        .or(cluster.cluster_id.clone())
                        .unwrap_or_else(|| "—".into());
                    let peer_key = conv
                        .as_ref()
                        .map(|c| c.peer_endpoint.as_str())
                        .unwrap_or(selected.as_str());
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(peer_key)
                            || n.node_id == peer_key
                            || n.node_id == selected
                    }) {
                        view.default_title = format!("{} · {}", node.os, node.hostname);
                        view.node_id = node.node_id.clone();
                        if view.peer_endpoint.is_empty() {
                            view.peer_endpoint =
                                remote_desktop_peer_identity(None, Some(node)).unwrap_or_default();
                        }
                        if view.peer_user_id.is_empty() {
                            view.peer_user_id = node.user_id.clone().unwrap_or_default();
                        }
                        view.os_label = node.os.clone();
                        view.online = node.online;
                        view.last_seen = node.last_seen;
                        view.remark = remarks.get(&node.node_id).cloned().unwrap_or_default();
                        view.title = display_name_with_remark(Some(view.remark.as_str()), || {
                            view.default_title.clone()
                        });
                    } else {
                        view.default_title = conv
                            .as_ref()
                            .and_then(|c| c.title.clone().or(c.peer_display_name.clone()))
                            .unwrap_or_else(|| wormhole_i18n::t("chat.unknown_device"));
                        view.title = view.default_title.clone();
                        view.node_id = peer_key.to_string();
                        view.os_label = "—".into();
                        view.online = false;
                        view.remark = remarks.get(&view.node_id).cloned().unwrap_or_default();
                        if !view.remark.is_empty() {
                            view.title = view.remark.clone();
                        }
                    }
                }
                view.status = format_last_seen(view.online, view.last_seen);
                view.is_contact = contact_matches_peer(
                    &contacts,
                    view.peer_user_id.as_str(),
                    view.node_id.as_str(),
                    view.peer_endpoint.as_str(),
                );
                ctx.notify();
            },
        );
    }

    fn persist_remark(&mut self, ctx: &mut ViewContext<Self>) {
        if self.node_id.is_empty() {
            return;
        }
        let node_id = self.node_id.clone();
        let conv_id = self.conv_id.clone();
        let remark = self
            .remark
            .chars()
            .take(MAX_REMARK_CHARS)
            .collect::<String>();
        let remark_for_save = remark.trim().to_string();
        self.remark = remark_for_save.clone();
        self.title =
            display_name_with_remark(Some(self.remark.as_str()), || self.default_title.clone());
        self.remark_dirty = false;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let remarks = set_device_remark(
                    &state.data_dir,
                    &node_id,
                    if remark_for_save.is_empty() {
                        None
                    } else {
                        Some(remark_for_save.as_str())
                    },
                )
                .await?;
                if let Some(conv_id) = conv_id {
                    let peer_display_name = if remark_for_save.is_empty() {
                        None
                    } else {
                        Some(remark_for_save.clone())
                    };
                    let _ = chat_set_peer_display_name(
                        &state,
                        SetChatPeerDisplayNameParams {
                            conv_id,
                            peer_display_name,
                        },
                    )
                    .await;
                }
                Ok::<BTreeMap<String, String>, String>(remarks)
            },
            |view, output, ctx| {
                if let Err(e) = output {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            format!("备注保存失败: {e}"),
                            StatusTone::Danger,
                        );
                    }
                }
                ctx.notify();
            },
        );
    }

    fn profile_action(
        &self,
        label: &str,
        icon_path: &'static str,
        action: ChatProfileAction,
        automation_id: &'static str,
    ) -> Box<dyn Element> {
        EventHandler::new(
            Container::new(
                Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(icons::chat_header_icon(icon_path, theme::muted()))
                    .with_child(
                        Container::new(
                            ui_text::chat_sidebar_time(label.to_string(), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .with_margin_top(6.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .finish(),
        )
        .with_automation_label(label.to_string())
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn profile_row(&self, label: &str, value: &str) -> Box<dyn Element> {
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                Align::new(
                    ui_text::mono(value.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .right()
                .finish(),
            )
            .finish()
    }

    fn profile_section(&self, title: &str, rows: Vec<(String, String)>) -> Box<dyn Element> {
        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            Container::new(ui_title(title.to_string(), self.font))
                .with_margin_bottom(8.0)
                .finish(),
        );
        for (label, value) in rows {
            col.add_child(
                Container::new(self.profile_row(&label, &value))
                    .with_vertical_padding(4.0)
                    .finish(),
            );
        }
        Container::new(col.finish())
            .with_padding_left(16.0)
            .with_padding_right(16.0)
            .with_padding_top(12.0)
            .with_padding_bottom(12.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn remark_section(&self) -> Box<dyn Element> {
        let draft = self.remark.clone();
        let marked = self.remark_field.marked_text.clone();
        let border = if self.remark_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let input = wrap_text_field_focus_on_click(
            TextFieldInput::builder(
                render_field_with_caret(
                    &draft,
                    &marked,
                    &wormhole_i18n::t("chat.profile.remark_placeholder"),
                    self.font,
                    self.remark_focused,
                    false,
                    self.caret_blink.visible,
                    self.remark_field.cursor,
                ),
                |ctx, action| {
                    ctx.dispatch_typed_action(ChatProfileAction::RemarkEdit(action));
                },
            )
            .focused(self.remark_focused)
            .finish(),
            |ctx| ctx.dispatch_typed_action(ChatProfileAction::FocusRemark),
        );

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        col.add_child(
            Container::new(ui_title(wormhole_i18n::t("chat.profile.device"), self.font))
                .with_margin_bottom(8.0)
                .finish(),
        );
        let mut remark_row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        remark_row.add_child(
            ui_text::body(wormhole_i18n::t("chat.profile.remark"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        remark_row.add_child(
            Container::new(
                ConstrainedBox::new(
                    Container::new(input)
                        .with_padding_left(10.0)
                        .with_padding_right(10.0)
                        .with_padding_top(6.0)
                        .with_padding_bottom(6.0)
                        .with_background(theme::canvas())
                        .with_border(Border::all(1.0).with_border_fill(border))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                        .finish(),
                )
                .with_min_width(120.0)
                .finish(),
            )
            .with_margin_left(10.0)
            .finish(),
        );
        col.add_child(
            Container::new(remark_row.finish())
                .with_vertical_padding(4.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.profile_row(
                &wormhole_i18n::t("chat.profile.os"),
                &self.os_label,
            ))
            .with_vertical_padding(4.0)
            .finish(),
        );
        Container::new(col.finish())
            .with_padding_left(16.0)
            .with_padding_right(16.0)
            .with_padding_top(12.0)
            .with_padding_bottom(12.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn spawn_set_mute(&mut self, muted: bool, ctx: &mut ViewContext<Self>) {
        let Some(conv_id) = self.conv_id.clone() else {
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                set_chat_muted_until(
                    &state.data_dir,
                    &conv_id,
                    muted.then_some(MUTE_FOREVER),
                )
                .await
            },
            move |view, output, ctx| match output {
                Ok(_) => {
                    view.muted = muted;
                    if let Ok(mut state) = view.shell_state.lock() {
                        let key = if muted {
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
    }

    fn spawn_add_contact(&mut self, ctx: &mut ViewContext<Self>) {
        let display_name = if self.title.trim().is_empty() {
            self.username_value()
        } else {
            self.title.clone()
        };
        let wormhole_id = if !self.peer_endpoint.is_empty() {
            self.peer_endpoint.clone()
        } else {
            self.node_id.clone()
        };
        let user_id = self.peer_user_id.clone();
        if display_name.trim().is_empty() && wormhole_id.trim().is_empty() && user_id.trim().is_empty()
        {
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(
                    wormhole_i18n::t("chat.toast.select_conversation"),
                    StatusTone::Muted,
                );
            }
            ctx.notify();
            return;
        }
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                chat_contacts_add(
                    &state.data_dir,
                    ChatContactAddParams {
                        display_name,
                        wormhole_id,
                        email: String::new(),
                        user_id,
                    },
                )
                .await
            },
            |view, output, ctx| match output {
                Ok(_) => {
                    view.is_contact = true;
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t("chat.toast.contact_added"),
                            StatusTone::Success,
                        );
                    }
                    ctx.notify();
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t_args(
                                "chat.toast.contact_add_failed",
                                &[("err", &err.to_string())],
                            ),
                            StatusTone::Danger,
                        );
                    }
                    ctx.notify();
                }
            },
        );
    }

    fn spawn_block(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(conv_id) = self.conv_id.clone() else {
            return;
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                set_chat_hidden(&state.data_dir, &conv_id, true).await
            },
            |view, output, ctx| match output {
                Ok(_) => {
                    if let Ok(mut guard) = view.selection.lock() {
                        *guard = None;
                    }
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.profile_open = false;
                        state.show_toast(
                            wormhole_i18n::t("chat.toast.conversation_deleted"),
                            StatusTone::Muted,
                        );
                        state.clear_pending_open();
                        state.bump_selection_tick();
                        state.bump_prefs_tick();
                    }
                    ctx.notify();
                }
                Err(err) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(
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
    }
}

fn format_last_seen(online: bool, last_seen: u64) -> String {
    if online {
        return wormhole_i18n::t("chat.presence.online");
    }
    if last_seen == 0 {
        return wormhole_i18n::t("chat.presence.unknown");
    }
    let Some(dt) = chrono::DateTime::from_timestamp(last_seen as i64, 0) else {
        return wormhole_i18n::t("chat.presence.offline");
    };
    let local = dt.with_timezone(&chrono::Local);
    let now = chrono::Local::now().date_naive();
    let date = local.date_naive();
    let time = local.format("%H:%M").to_string();
    if date == now {
        wormhole_i18n::t_args("chat.presence.last_seen_today", &[("time", &time)])
    } else if date + chrono::Duration::days(1) == now {
        wormhole_i18n::t_args("chat.presence.last_seen_yesterday", &[("time", &time)])
    } else {
        let day = local.format("%Y/%m/%d").to_string();
        wormhole_i18n::t_args(
            "chat.presence.last_seen_on",
            &[("date", &day), ("time", &time)],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::format_last_seen;

    #[test]
    fn last_seen_online_ignores_timestamp() {
        let label = format_last_seen(true, 1);
        assert!(!label.is_empty());
    }
}

impl Entity for ChatProfilePanelView {
    type Event = ChatProfileEvent;
}

impl View for ChatProfilePanelView {
    fn ui_name() -> &'static str {
        "ChatProfilePanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|state| state.profile_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }

        let status_color = if self.online {
            theme::accent_cool()
        } else {
            theme::muted()
        };
        let mute_label = if self.muted {
            wormhole_i18n::t("chat.profile.unmute")
        } else {
            wormhole_i18n::t("chat.profile.mute")
        };

        let close = EventHandler::new(
            Container::new(
                ui_text::section_title("×".to_string(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_uniform_padding(8.0)
            .finish(),
        )
        .with_automation_label(wormhole_i18n::t("chat.profile.close"))
        .with_automation_id("chat:profile_close")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatProfileAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let identity = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(tg_avatar(
                if self.os_label.is_empty() || self.os_label == "—" {
                    chat_avatar_for_os("")
                } else {
                    chat_avatar_for_os(&self.os_label)
                },
                self.font,
                TG_AVATAR_LG_SIZE,
            ))
            .with_child(
                Container::new(
                    ui_text::section_title(self.title.clone(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_top(12.0)
                .with_margin_bottom(4.0)
                .finish(),
            )
            .with_child(
                Container::new(
                    ui_text::body(self.status.clone(), self.font)
                        .with_color(status_color)
                        .finish(),
                )
                .with_margin_top(4.0)
                .finish(),
            )
            .finish();

        let mut head_stack = Stack::new();
        head_stack.add_child(
            Container::new(identity)
                .with_padding_left(16.0)
                .with_padding_right(16.0)
                .with_padding_top(20.0)
                .with_padding_bottom(16.0)
                .finish(),
        );
        head_stack.add_child(Align::new(close).top_left().finish());
        let head = Container::new(head_stack.finish())
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish();

        let actions = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(self.profile_action(
                    &wormhole_i18n::t("chat.profile.message"),
                    "chat-compose-send.svg",
                    ChatProfileAction::Message,
                    "chat:profile_message",
                ))
                .with_child(self.profile_action(
                    &mute_label,
                    "chat-header-search.svg",
                    ChatProfileAction::Mute,
                    "chat:profile_mute",
                ))
                .with_child(self.profile_action(
                    &wormhole_i18n::t("chat.menu.remote_desktop"),
                    "chat-header-rdp.svg",
                    ChatProfileAction::RemoteDesktop,
                    "chat:profile_rdp",
                ))
                .finish(),
        )
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .finish();

        let username = self.username_value();
        let username_row = EventHandler::new(self.profile_row(
            &wormhole_i18n::t("chat.profile.username"),
            &username,
        ))
        .with_automation_label(wormhole_i18n::t("chat.profile.copy_username"))
        .with_automation_id("chat:profile_username")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatProfileAction::CopyUsername);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut details = Flex::column().with_main_axis_size(MainAxisSize::Min);
        details.add_child(
            Container::new(username_row)
                .with_padding_left(16.0)
                .with_padding_right(16.0)
                .with_padding_top(12.0)
                .with_padding_bottom(8.0)
                .finish(),
        );
        if !self.is_contact {
            details.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body(wormhole_i18n::t("chat.profile.add_contact"), self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .with_padding_left(16.0)
                    .with_padding_right(16.0)
                    .with_padding_bottom(12.0)
                    .finish(),
                )
                .with_automation_label(wormhole_i18n::t("chat.profile.add_contact"))
                .with_automation_id("chat:profile_add_contact")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ChatProfileAction::AddContact);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            );
        }

        let cluster_section = self.profile_section(
            &wormhole_i18n::t("chat.profile.cluster"),
            vec![(
                wormhole_i18n::t("chat.profile.cluster"),
                self.cluster_name.clone(),
            )],
        );

        let block = EventHandler::new(
            Container::new(
                ui_text::body(wormhole_i18n::t("chat.profile.block"), self.font)
                    .with_color(theme::danger())
                    .finish(),
            )
            .with_padding_left(16.0)
            .with_padding_right(16.0)
            .with_padding_top(16.0)
            .with_padding_bottom(16.0)
            .finish(),
        )
        .with_automation_label(wormhole_i18n::t("chat.profile.block"))
        .with_automation_id("chat:profile_block")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChatProfileAction::Block);
            DispatchEventResult::StopPropagation
        })
        .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(head)
            .with_child(actions)
            .with_child(details.finish())
            .with_child(cluster_section)
            .with_child(self.remark_section())
            .with_child(block)
            .finish()
    }
}

impl TypedActionView for ChatProfilePanelView {
    type Action = ChatProfileAction;

    fn handle_action(&mut self, action: &ChatProfileAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChatProfileAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.profile_open = false;
                    state.bump_overlay_tick();
                }
                ctx.notify();
            }
            ChatProfileAction::Message => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.request_compose_focus();
                }
                ctx.emit(ChatProfileEvent::FocusCompose);
                ctx.notify();
            }
            ChatProfileAction::Mute => {
                self.spawn_set_mute(!self.muted, ctx);
            }
            ChatProfileAction::RemoteDesktop => {
                let peer = self.peer_endpoint.trim().to_string();
                if peer.is_empty() {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t("chat.toast.no_rdp_peer"),
                            StatusTone::Muted,
                        );
                    }
                    ctx.notify();
                    return;
                }
                if !self.online {
                    if let Ok(mut state) = self.shell_state.lock() {
                        state.show_toast(
                            wormhole_i18n::t("chat.toast.peer_offline_rdp"),
                            StatusTone::Muted,
                        );
                    }
                    ctx.notify();
                    return;
                }
                if let Ok(mut state) = self.shell_state.lock() {
                    state.show_toast(
                        wormhole_i18n::t("chat.toast.opening_rdp"),
                        StatusTone::Neutral,
                    );
                }
                ctx.emit(ChatProfileEvent::OpenRemoteDesktop { peer });
                self.refresh(ctx);
            }
            ChatProfileAction::CopyUsername => {
                let value = self.username_value();
                if value.is_empty() {
                    return;
                }
                match write_clipboard_text(&value) {
                    Ok(()) => {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.show_toast(
                                wormhole_i18n::t("chat.toast.username_copied"),
                                StatusTone::Success,
                            );
                        }
                    }
                    Err(err) => {
                        if let Ok(mut state) = self.shell_state.lock() {
                            state.show_toast(err, StatusTone::Danger);
                        }
                    }
                }
                ctx.notify();
            }
            ChatProfileAction::AddContact => self.spawn_add_contact(ctx),
            ChatProfileAction::Block => self.spawn_block(ctx),
            ChatProfileAction::FocusRemark => {
                self.remark_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChatProfileAction::RemarkEdit(edit) => {
                self.remark_field.apply(&mut self.remark, edit);
                if self.remark.chars().count() > MAX_REMARK_CHARS {
                    self.remark = self.remark.chars().take(MAX_REMARK_CHARS).collect();
                }
                self.remark_dirty = true;
                self.title = display_name_with_remark(Some(self.remark.as_str()), || {
                    self.default_title.clone()
                });
                sync_caret_blink(self, ctx);
                self.persist_remark(ctx);
                ctx.notify();
            }
        }
    }
}

impl CaretBlinkHost for ChatProfilePanelView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.remark_focused
    }
}
