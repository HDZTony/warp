//! Contacts modal — design §7.5 / desktop-current.html `#chat-contacts-panel`.

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{tg_avatar, StatusTone, HUD_RADIUS};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click_with_label,
    CaretBlink, CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_start_contact_conversation, chat_start_conversation, lookup_account_by_email,
    peer_endpoints_for_user_id, StartChatContactParams, StartChatConversationParams,
};
use wormhole_desktop_core::chat_contacts::{
    aggregate_cluster_contacts_by_account, chat_contacts_add, chat_contacts_list_manual,
    chat_contacts_set_user_id, classify_contact_id_input, merge_contact_rows, ChatContactAddParams,
    ContactDto, ContactIdInput,
};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::load_device_remarks;

const DIALOG_WIDTH: f32 = 420.0;
const DIALOG_HEIGHT: f32 = 560.0;
const ADD_DIALOG_WIDTH: f32 = 360.0;
const ADD_FIELD_HEIGHT: f32 = 44.0;

#[derive(Debug, Clone)]
pub enum ContactsPanelAction {
    Close,
    ToggleSort,
    SearchEdit(TextFieldEditAction),
    FocusSearch,
    ActivateSearch,
    Select(String),
    OpenAdd,
    CloseAdd,
    FocusAddName,
    FocusAddId,
    AddNameEdit(TextFieldEditAction),
    AddIdEdit(TextFieldEditAction),
    SubmitAdd,
    Refresh,
}

#[derive(Debug, Clone)]
pub enum ContactsPanelEvent {
    OpenConversation(String),
    Closed,
}

pub struct ContactsPanelView {
    core: CoreHandle,
    shell_state: SharedChatShellState,
    font: FamilyId,
    emoji_font: FamilyId,
    contacts: Vec<ContactDto>,
    search: String,
    search_field: TextFieldState,
    search_focused: bool,
    caret_blink: CaretBlink,
    sort_desc: bool,
    scroll: ClippedScrollStateHandle,
    add_name: String,
    add_name_field: TextFieldState,
    add_id: String,
    add_id_field: TextFieldState,
    /// 0 = name, 1 = id/email, 255 = none
    add_focused: u8,
    status: String,
    status_tone: StatusTone,
    last_open: bool,
}

impl ContactsPanelView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let emoji_font = crate::ui::fonts::load_emoji_font(ctx);
        let view = Self {
            core,
            shell_state,
            font,
            emoji_font,
            contacts: Vec::new(),
            search: String::new(),
            search_field: TextFieldState::new(),
            search_focused: true,
            caret_blink: CaretBlink::new(),
            sort_desc: false,
            scroll: ClippedScrollStateHandle::new(),
            add_name: String::new(),
            add_name_field: TextFieldState::new(),
            add_id: String::new(),
            add_id_field: TextFieldState::new(),
            add_focused: 255,
            status: String::new(),
            status_tone: StatusTone::Neutral,
            last_open: false,
        };
        view.poll_open(ctx);
        view
    }

    fn poll_open(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            },
            |view, _, ctx| {
                let open = view
                    .shell_state
                    .lock()
                    .map(|s| s.contacts_open)
                    .unwrap_or(false);
                if open && !view.last_open {
                    view.last_open = true;
                    view.search_focused = true;
                    view.add_focused = 255;
                    view.reload(ctx);
                    sync_caret_blink(view, ctx);
                } else if !open {
                    view.last_open = false;
                    view.search_focused = false;
                    view.add_focused = 255;
                    sync_caret_blink(view, ctx);
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
                let manual = chat_contacts_list_manual(&state.data_dir)
                    .await
                    .unwrap_or_default();
                let remarks = load_device_remarks(&state.data_dir).await.unwrap_or_default();
                let cluster = cluster_status_hud(&state).await.ok();
                let local_endpoint = state
                    .manager
                    .node_if_ready()
                    .map(|n| n.endpoint_id().to_string())
                    .unwrap_or_default();
                let cluster_rows = if let Some(status) = cluster {
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
                merge_contact_rows(cluster_rows, manual)
            },
            |view, rows, ctx| {
                view.contacts = rows;
                view.status.clear();
                ctx.notify();
            },
        );
    }

    fn filtered(&self) -> Vec<ContactDto> {
        let q = self.search.trim().to_lowercase();
        let mut rows: Vec<_> = self
            .contacts
            .iter()
            .filter(|c| {
                if q.is_empty() {
                    return true;
                }
                c.display_name.to_lowercase().contains(&q)
                    || c.wormhole_id.to_lowercase().contains(&q)
                    || c.email.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
        rows.sort_by(|a, b| {
            let cmp = a
                .display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase());
            if self.sort_desc {
                cmp.reverse()
            } else {
                cmp
            }
        });
        rows
    }

    fn open_contact(&mut self, id: &str, ctx: &mut ViewContext<Self>) {
        let Some(contact) = self.contacts.iter().find(|c| c.id == id).cloned() else {
            return;
        };
        let core = self.core.clone();
        let contact_id = contact.id.clone();
        let name = contact.display_name.clone();
        let email = contact.email.clone();
        let mut user_id = contact.user_id.clone();
        let endpoints = contact.endpoints.clone();
        let bootstraps = contact.endpoint_bootstraps.clone();
        let fallback_peer = contact.wormhole_id.clone();
        let needs_lookup = user_id
            .as_ref()
            .map(|id| id.trim().is_empty())
            .unwrap_or(true)
            && fallback_peer.trim().is_empty()
            && email.contains('@');
        if !contact.can_chat && !needs_lookup {
            self.status = "该联系人无法打开私聊。请补充帐号 ID 或 Wormhole ID。".into();
            self.status_tone = StatusTone::Warn;
            ctx.notify();
            return;
        }
        self.status = if needs_lookup {
            "正在解析邮箱帐号…".into()
        } else {
            String::new()
        };
        self.status_tone = StatusTone::Muted;
        ctx.notify();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                if needs_lookup {
                    let looked = lookup_account_by_email(&runtime.state, &email).await?;
                    let updated = chat_contacts_set_user_id(
                        &runtime.state.data_dir,
                        &contact_id,
                        &looked.user_id,
                    )
                    .await?;
                    user_id = updated.user_id;
                }
                if let Some(peer_user_id) = user_id.filter(|id| !id.trim().is_empty()) {
                    let mut peer_endpoints = if endpoints.is_empty() {
                        if fallback_peer.is_empty() {
                            Vec::new()
                        } else {
                            vec![(fallback_peer, Vec::new())]
                        }
                    } else {
                        endpoints
                            .into_iter()
                            .zip(
                                bootstraps
                                    .into_iter()
                                    .chain(std::iter::repeat_with(Vec::new)),
                            )
                            .collect::<Vec<_>>()
                    };
                    if peer_endpoints.is_empty() {
                        peer_endpoints =
                            peer_endpoints_for_user_id(&runtime.state, &peer_user_id).await;
                    }
                    return chat_start_contact_conversation(
                        &runtime.ctx,
                        &runtime.state,
                        StartChatContactParams {
                            peer_user_id,
                            peer_display_name: Some(name),
                            peer_endpoints,
                        },
                    )
                    .await;
                }
                if fallback_peer.is_empty() {
                    return Err("该联系人仅有邮箱，无法打开私聊。请补充用户 ID 或 Wormhole ID。".into());
                }
                chat_start_conversation(
                    &runtime.ctx,
                    &runtime.state,
                    StartChatConversationParams {
                        backend: None,
                        peer: Some(fallback_peer.clone()),
                        peer_endpoint: Some(fallback_peer),
                        peer_display_name: Some(name),
                        peer_bootstrap_addrs: Vec::new(),
                    },
                )
                .await
            },
            |view, result, ctx| match result {
                Ok(conv) => {
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.close_contacts();
                    }
                    view.status.clear();
                    ctx.emit(ContactsPanelEvent::OpenConversation(conv.id));
                    view.reload(ctx);
                    ctx.notify();
                }
                Err(err) => {
                    view.status = err.clone();
                    view.status_tone = StatusTone::Danger;
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.show_toast(err, StatusTone::Danger);
                    }
                    view.reload(ctx);
                    ctx.notify();
                }
            },
        );
    }

    fn submit_add(&mut self, ctx: &mut ViewContext<Self>) {
        let display_name = self.add_name.trim().to_string();
        let id_raw = self.add_id.trim().to_string();
        if display_name.is_empty() || id_raw.is_empty() {
            self.status = "请填写联系人名称和用户 ID / Wormhole ID / 邮箱".into();
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        let Some(classified) = classify_contact_id_input(&id_raw) else {
            self.status = "请填写联系人名称和用户 ID / Wormhole ID / 邮箱".into();
            self.status_tone = StatusTone::Danger;
            ctx.notify();
            return;
        };
        let (wormhole_id, email, mut user_id) = match classified {
            ContactIdInput::Email(email) => (String::new(), email, String::new()),
            ContactIdInput::UserId(user_id) => (String::new(), String::new(), user_id),
            ContactIdInput::WormholeId(wormhole_id) => (wormhole_id, String::new(), String::new()),
        };
        let core = self.core.clone();
        self.status = if email.contains('@') {
            "正在解析邮箱帐号…".into()
        } else {
            String::new()
        };
        self.status_tone = StatusTone::Muted;
        ctx.notify();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let mut lookup_warning = None;
                if email.contains('@') && user_id.is_empty() {
                    match lookup_account_by_email(&runtime.state, &email).await {
                        Ok(looked) => user_id = looked.user_id,
                        Err(err) => lookup_warning = Some(err),
                    }
                }
                let dto = chat_contacts_add(
                    &runtime.state.data_dir,
                    ChatContactAddParams {
                        display_name,
                        wormhole_id,
                        email,
                        user_id,
                    },
                )
                .await?;
                Ok((dto, lookup_warning))
            },
            |view, result, ctx| match result {
                Ok((dto, lookup_warning)) => {
                    view.add_name.clear();
                    view.add_id.clear();
                    view.add_name_field = TextFieldState::new();
                    view.add_id_field = TextFieldState::new();
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.contacts_add_open = false;
                    }
                    view.add_focused = 255;
                    view.search_focused = true;
                    sync_caret_blink(view, ctx);
                    view.reload(ctx);
                    if let Some(warning) = lookup_warning {
                        view.status = format!("已添加联系人，但暂无法私聊：{warning}");
                        view.status_tone = StatusTone::Warn;
                    } else if dto.can_chat {
                        view.status = "已添加联系人".into();
                        view.status_tone = StatusTone::Success;
                    } else {
                        view.status =
                            "已添加联系人，但缺少帐号 ID，暂无法打开私聊".into();
                        view.status_tone = StatusTone::Warn;
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
}

impl Entity for ContactsPanelView {
    type Event = ContactsPanelEvent;
}

impl View for ContactsPanelView {
    fn ui_name() -> &'static str {
        "ContactsPanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|s| s.contacts_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }
        let add_open = self
            .shell_state
            .lock()
            .map(|s| s.contacts_add_open)
            .unwrap_or(false);

        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 140))
                .finish(),
        )
        .with_automation_label("关闭联系人")
        .with_automation_id("chat:contacts_scrim")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let dialog = ConstrainedBox::new(self.dialog_body(add_open))
            .with_width(DIALOG_WIDTH)
            .with_height(DIALOG_HEIGHT)
            .finish();

        let dialog = EventHandler::new(dialog)
            .with_automation_label("联系人")
            .with_automation_id("chat:contacts_dialog")
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

impl ContactsPanelView {
    fn dialog_body(&self, add_open: bool) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        // Head
        let mut head = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        head.add_child(Expanded::new(1.0, ui_text::body("联系人".to_string(), self.font)
                .with_color(theme::text())
                .finish()).finish());
        head.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::chat_preview(
                        if self.sort_desc { "Z→A" } else { "A→Z" }.to_string(),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_uniform_padding(8.0)
                .finish(),
            )
            .with_automation_label(if self.sort_desc { "Z→A" } else { "A→Z" })
            .with_automation_id("chat:contacts_sort")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::ToggleSort);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        col.add_child(
            Container::new(head.finish())
                .with_padding_left(18.0)
                .with_padding_right(12.0)
                .with_padding_top(14.0)
                .with_padding_bottom(10.0)
                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                .finish(),
        );

        // Search — HTML `.chat-contacts-search`
        col.add_child(self.search_box());

        // Body
        let rows = self.filtered();
        let body: Box<dyn Element> = if rows.is_empty() {
            Align::new(
                ui_text::chat_preview("未找到相关联系人".to_string(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish()
        } else {
            let mut list = Flex::column().with_main_axis_size(MainAxisSize::Min);
            for c in rows {
                list.add_child(self.contact_row(&c));
            }
            ClippedScrollable::vertical(
                self.scroll.clone(),
                list.finish(),
                ScrollbarWidth::Auto,
                Fill::None,
                Fill::None,
                Fill::None,
            )
            .finish()
        };
        col.add_child(Expanded::new(1.0, body).finish());

        if !self.status.is_empty() && !add_open {
            col.add_child(
                Container::new(
                    ui_text::chat_preview(self.status.clone(), self.font)
                        .with_color(match self.status_tone {
                            StatusTone::Danger => theme::danger(),
                            StatusTone::Success => theme::accent_cool(),
                            _ => theme::muted(),
                        })
                        .finish(),
                )
                .with_padding_left(18.0)
                .with_padding_right(18.0)
                .with_padding_bottom(6.0)
                .finish(),
            );
        }

        // Foot
        let mut foot = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        foot.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("添加联系人".to_string(), self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .finish(),
            )
            .with_automation_label("添加联系人")
            .with_automation_id("chat:contacts_add")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::OpenAdd);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        foot.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        foot.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("关闭".to_string(), self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .finish(),
            )
            .with_automation_label("关闭")
            .with_automation_id("chat:contacts_close")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::Close);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        col.add_child(
            Container::new(foot.finish())
                .with_padding_left(12.0)
                .with_padding_right(12.0)
                .with_border(Border::top(1.0).with_border_fill(theme::border()))
                .finish(),
        );

        let panel = Container::new(col.finish())
            .with_background(theme::panel())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 2.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish();

        if add_open {
            Stack::new()
                .with_child(panel)
                .with_child(self.add_form_overlay())
                .finish()
        } else {
            panel
        }
    }

    fn contact_row(&self, c: &ContactDto) -> Box<dyn Element> {
        let id = c.id.clone();
        let avatar = tg_avatar(
            c.display_name.chars().take(2).collect::<String>(),
            self.font,
            40.0,
        );
        let mut copy = Flex::column().with_main_axis_size(MainAxisSize::Min);
        copy.add_child(
            ui_text::body(c.display_name.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        let subtitle = if c.device_count > 0 {
            if !c.email.is_empty() {
                format!("{} · {} 台设备", c.email, c.device_count)
            } else {
                format!("{} 台设备", c.device_count)
            }
        } else if !c.email.is_empty() {
            c.email.clone()
        } else if let Some(user_id) = c.user_id.as_ref() {
            user_id.clone()
        } else {
            c.wormhole_id.clone()
        };
        copy.add_child(
            ui_text::chat_preview(subtitle, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(avatar)
            .with_child(
                Container::new(copy.finish())
                    .with_padding_left(10.0)
                    .finish(),
            )
            .finish();
        let contact_name = c.display_name.clone();
        EventHandler::new(
            Container::new(row)
                .with_padding_left(10.0)
                .with_padding_right(10.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
        )
        .with_automation_label(contact_name)
        .with_automation_id(format!("chat:contact:{id}"))
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::Select(id.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let border = if self.search_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let field = render_search_field_with_caret(
            &self.search,
            &self.search_field.marked_text,
            "搜索",
            self.font,
            self.search_focused,
            false,
            self.caret_blink.visible,
            self.search_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ContactsPanelAction::SearchEdit(action));
        })
        .focused(self.search_focused)
        .ime_preedit(!self.search_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click_with_label(input, "搜索", |ctx| {
            ctx.dispatch_typed_action(ContactsPanelAction::ActivateSearch);
        });
        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_margin_right(10.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();
        ConstrainedBox::new(
            Container::new(row)
                .with_padding_left(18.0)
                .with_padding_right(18.0)
                .with_background(theme::canvas())
                .with_border(Border::bottom(1.0).with_border_fill(border))
                .finish(),
        )
        .with_min_height(50.0)
        .finish()
    }

    fn add_form_overlay(&self) -> Box<dyn Element> {
        // HTML `.chat-contact-modal` — dim scrim + centered dialog
        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 184))
                .finish(),
        )
        .with_automation_label("关闭添加联系人")
        .with_automation_id("chat:contacts_add_scrim")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::CloseAdd);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut head = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        head.add_child(
            Expanded::new(
                1.0,
                ui_text::body("添加联系人".to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        );
        head.add_child(
            EventHandler::new(
                ConstrainedBox::new(
                    Align::new(
                        ui_text::body("×".to_string(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .finish(),
                )
                .with_width(44.0)
                .with_height(44.0)
                .finish(),
            )
            .with_automation_label("关闭添加联系人弹框")
            .with_automation_id("chat:contacts_add_close")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::CloseAdd);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );

        let mut form = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        form.add_child(self.add_field(
            "联系人名称",
            "输入联系人名称",
            &self.add_name,
            &self.add_name_field,
            0,
            ContactsPanelAction::FocusAddName,
            ContactsPanelAction::AddNameEdit,
        ));
        form.add_child(
            Container::new(self.add_field(
                "用户 ID / Wormhole ID / 邮箱",
                "输入用户 ID / Wormhole ID / 邮箱",
                &self.add_id,
                &self.add_id_field,
                1,
                ContactsPanelAction::FocusAddId,
                ContactsPanelAction::AddIdEdit,
            ))
            .with_padding_top(14.0)
            .finish(),
        );

        let feedback = if self.status.is_empty() {
            " ".to_string()
        } else {
            self.status.clone()
        };
        let feedback_color = if self.status.is_empty() {
            theme::muted()
        } else {
            match self.status_tone {
                StatusTone::Danger => theme::danger(),
                StatusTone::Success => theme::accent_cool(),
                _ => theme::muted(),
            }
        };
        form.add_child(
            Container::new(
                ui_text::chat_preview(feedback, self.font)
                    .with_color(feedback_color)
                    .finish(),
            )
            .with_padding_top(12.0)
            .finish(),
        );

        let cancel_btn = EventHandler::new(
            Container::new(
                ConstrainedBox::new(
                    Align::new(
                        ui_text::body("取消".to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_min_height(ADD_FIELD_HEIGHT)
                .finish(),
            )
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .finish(),
        )
        .with_automation_label("取消")
        .with_automation_id("chat:contacts_add_cancel")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::CloseAdd);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let save_btn = EventHandler::new(
            Container::new(
                ConstrainedBox::new(
                    Align::new(
                        ui_text::body("添加".to_string(), self.font)
                            .with_color(theme::canvas())
                            .finish(),
                    )
                    .finish(),
                )
                .with_min_height(ADD_FIELD_HEIGHT)
                .finish(),
            )
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_background(theme::accent_cool())
            .with_border(Border::all(1.0).with_border_fill(theme::accent_cool()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .with_margin_left(7.0)
            .finish(),
        )
        .with_automation_label("添加")
        .with_automation_id("chat:contacts_add_save")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::SubmitAdd);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut actions = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::End)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        actions.add_child(cancel_btn);
        actions.add_child(save_btn);
        form.add_child(Container::new(actions.finish()).with_padding_top(4.0).finish());

        let dialog = EventHandler::new(
            ConstrainedBox::new(
                Container::new(
                    Flex::column()
                        .with_main_axis_size(MainAxisSize::Min)
                        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                        .with_child(
                            Container::new(head.finish())
                                .with_padding_left(18.0)
                                .with_padding_right(10.0)
                                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                                .finish(),
                        )
                        .with_child(
                            Container::new(form.finish())
                                .with_uniform_padding(18.0)
                                .finish(),
                        )
                        .finish(),
                )
                .with_background(theme::panel())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                .finish(),
            )
            .with_width(ADD_DIALOG_WIDTH)
            .finish(),
        )
        .with_automation_label("添加联系人表单")
        .with_automation_id("chat:contacts_add_dialog")
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish();

        Stack::new()
            .with_child(scrim)
            .with_child(Align::new(dialog).finish())
            .finish()
    }

    fn add_field(
        &self,
        label: &str,
        placeholder: &str,
        value: &str,
        field_state: &TextFieldState,
        focus_idx: u8,
        focus_action: ContactsPanelAction,
        edit_ctor: fn(TextFieldEditAction) -> ContactsPanelAction,
    ) -> Box<dyn Element> {
        let focused = self.add_focused == focus_idx;
        let border = if focused {
            theme::border_bright()
        } else {
            theme::border()
        };
        let field = render_search_field_with_caret(
            value,
            &field_state.marked_text,
            placeholder,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
            field_state.cursor,
        );
        let input = TextFieldInput::builder(field, move |ctx, action| {
            ctx.dispatch_typed_action(edit_ctor(action));
        })
        .focused(focused)
        .ime_preedit(!field_state.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click_with_label(
            ConstrainedBox::new(
                Container::new(input)
                    .with_padding_left(10.0)
                    .with_padding_right(10.0)
                    .with_background(theme::canvas())
                    .with_border(Border::all(1.0).with_border_fill(border))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                    .finish(),
            )
            .with_min_height(ADD_FIELD_HEIGHT)
            .finish(),
            placeholder,
            move |ctx| {
                ctx.dispatch_typed_action(focus_action.clone());
            },
        );
        Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Container::new(
                    ui_text::chat_preview(label.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_padding_bottom(7.0)
                .finish(),
            )
            .with_child(input)
            .finish()
    }
}

impl TypedActionView for ContactsPanelView {
    type Action = ContactsPanelAction;

    fn handle_action(&mut self, action: &ContactsPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            ContactsPanelAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.close_contacts();
                }
                ctx.emit(ContactsPanelEvent::Closed);
                ctx.notify();
            }
            ContactsPanelAction::ToggleSort => {
                self.sort_desc = !self.sort_desc;
                ctx.notify();
            }
            ContactsPanelAction::SearchEdit(edit) => {
                self.search_field.apply(&mut self.search, edit);
                self.search_focused = true;
                self.add_focused = 255;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::FocusSearch | ContactsPanelAction::ActivateSearch => {
                self.search_focused = true;
                self.add_focused = 255;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::Select(id) => self.open_contact(id, ctx),
            ContactsPanelAction::OpenAdd => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.contacts_add_open = true;
                }
                self.search_focused = false;
                self.add_focused = 0;
                self.status.clear();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::CloseAdd => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.contacts_add_open = false;
                }
                self.search_focused = true;
                self.add_focused = 255;
                self.status.clear();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::FocusAddName => {
                self.add_focused = 0;
                self.search_focused = false;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::FocusAddId => {
                self.add_focused = 1;
                self.search_focused = false;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::AddNameEdit(edit) => {
                self.add_focused = 0;
                self.search_focused = false;
                self.add_name_field.apply(&mut self.add_name, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::AddIdEdit(edit) => {
                self.add_focused = 1;
                self.search_focused = false;
                self.add_id_field.apply(&mut self.add_id, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ContactsPanelAction::SubmitAdd => self.submit_add(ctx),
            ContactsPanelAction::Refresh => self.reload(ctx),
        }
    }
}

impl CaretBlinkHost for ContactsPanelView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused || self.add_focused < 2
    }
}
