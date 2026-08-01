//! Contacts modal — design §7.5 / desktop-current.html `#chat-contacts-panel`.

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    chat_search_pill, chat_sidebar_search_bg, tg_avatar, StatusTone, HUD_RADIUS,
};
use crate::ui::text_field_input::{
    render_search_field_with_caret, wrap_text_field_focus_on_click, CaretBlink, TextFieldEditAction,
    TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_start_conversation, StartChatConversationParams,
};
use wormhole_desktop_core::chat_contacts::{
    chat_contacts_add, chat_contacts_list_manual, merge_contact_rows, ChatContactAddParams,
    ContactDto, ContactSource,
};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;
use wormhole_desktop_core::device_remarks::{display_name_with_remark, load_device_remarks};

const DIALOG_WIDTH: f32 = 420.0;
const DIALOG_HEIGHT: f32 = 560.0;

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
    AddNameEdit(TextFieldEditAction),
    AddIdEdit(TextFieldEditAction),
    AddEmailEdit(TextFieldEditAction),
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
    add_email: String,
    add_email_field: TextFieldState,
    add_focused: u8,
    status: String,
    status_tone: StatusTone,
    opening: bool,
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
        let mut view = Self {
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
            add_email: String::new(),
            add_email_field: TextFieldState::new(),
            add_focused: 0,
            status: String::new(),
            status_tone: StatusTone::Neutral,
            opening: false,
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
                    view.opening = false;
                    view.reload(ctx);
                } else if !open {
                    view.last_open = false;
                    view.opening = false;
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
                let mut cluster_rows = Vec::new();
                if let Some(status) = cluster {
                    for node in status.nodes {
                        let endpoint = node
                            .chat_endpoint_id
                            .clone()
                            .unwrap_or_else(|| node.node_id.clone());
                        if endpoint.is_empty() || endpoint == local_endpoint {
                            continue;
                        }
                        let title = display_name_with_remark(
                            remarks.get(&node.node_id).map(String::as_str),
                            || {
                                if node.hostname.trim().is_empty() {
                                    endpoint.clone()
                                } else {
                                    format!("{} · {}", node.os, node.hostname)
                                }
                            },
                        );
                        cluster_rows.push(ContactDto {
                            id: format!("cluster:{endpoint}"),
                            display_name: title,
                            wormhole_id: endpoint,
                            email: String::new(),
                            source: ContactSource::Cluster,
                            can_chat: true,
                            bootstrap_addrs: node.chat_bootstrap_addrs.clone(),
                        });
                    }
                }
                merge_contact_rows(cluster_rows, manual)
            },
            |view, rows, ctx| {
                view.contacts = rows;
                if !view.opening {
                    view.status.clear();
                }
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
        if self.opening {
            return;
        }
        let Some(contact) = self.contacts.iter().find(|c| c.id == id).cloned() else {
            return;
        };
        if !contact.can_chat || contact.wormhole_id.is_empty() {
            self.status = "该联系人仅有邮箱，无法打开私聊。请补充 Wormhole ID。".into();
            self.status_tone = StatusTone::Warn;
            if let Ok(mut state) = self.shell_state.lock() {
                state.show_toast(
                    "该联系人仅有邮箱，无法打开私聊",
                    StatusTone::Warn,
                );
            }
            ctx.notify();
            return;
        }
        self.opening = true;
        self.status = "正在打开会话…".into();
        self.status_tone = StatusTone::Neutral;
        if let Ok(mut state) = self.shell_state.lock() {
            state.show_toast("正在打开会话…", StatusTone::Muted);
        }
        let core = self.core.clone();
        let peer = contact.wormhole_id.clone();
        let name = contact.display_name.clone();
        let bootstrap = contact.bootstrap_addrs.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                chat_start_conversation(
                    app,
                    &runtime.state,
                    StartChatConversationParams {
                        backend: None,
                        peer: Some(peer.clone()),
                        peer_endpoint: Some(peer),
                        peer_display_name: Some(name),
                        peer_bootstrap_addrs: bootstrap,
                    },
                )
                .await
            },
            |view, result, ctx| {
                view.opening = false;
                match result {
                    Ok(conv) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.close_contacts();
                            state.show_toast("已打开会话", StatusTone::Success);
                        }
                        view.status.clear();
                        ctx.emit(ContactsPanelEvent::OpenConversation(conv.id));
                        ctx.notify();
                    }
                    Err(err) => {
                        view.status = err.clone();
                        view.status_tone = StatusTone::Danger;
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(format!("无法打开会话: {err}"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn submit_add(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let params = ChatContactAddParams {
            display_name: self.add_name.clone(),
            wormhole_id: self.add_id.clone(),
            email: self.add_email.clone(),
        };
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                chat_contacts_add(&runtime.state.data_dir, params).await
            },
            |view, result, ctx| match result {
                Ok(_) => {
                    view.add_name.clear();
                    view.add_id.clear();
                    view.add_email.clear();
                    if let Ok(mut state) = view.shell_state.lock() {
                        state.contacts_add_open = false;
                    }
                    view.reload(ctx);
                    view.status = "已添加联系人".into();
                    view.status_tone = StatusTone::Success;
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

        // Search
        col.add_child(
            Container::new(self.search_box())
                .with_padding_left(12.0)
                .with_padding_right(12.0)
                .with_padding_top(10.0)
                .with_padding_bottom(8.0)
                .finish(),
        );

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
            let mut list = Flex::column()
                .with_main_axis_size(MainAxisSize::Min)
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
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

        if !self.status.is_empty() {
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
        let dimmed = self.opening;
        let avatar = tg_avatar(
            c.display_name.chars().take(2).collect::<String>(),
            self.font,
            40.0,
        );
        let mut copy = Flex::column().with_main_axis_size(MainAxisSize::Min);
        copy.add_child(
            ui_text::body(c.display_name.clone(), self.font)
                .with_color(if dimmed {
                    theme::muted()
                } else {
                    theme::text()
                })
                .finish(),
        );
        let subtitle = if !c.wormhole_id.is_empty() {
            c.wormhole_id.clone()
        } else {
            c.email.clone()
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
                Expanded::new(
                    1.0,
                    Container::new(copy.finish())
                        .with_padding_left(10.0)
                        .finish(),
                )
                .finish(),
            )
            .finish();
        EventHandler::new(
            Container::new(row)
                .with_padding_left(10.0)
                .with_padding_right(10.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(ContactsPanelAction::Select(id.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let field = render_search_field_with_caret(
            &self.search,
            &self.search_field.marked_text,
            "搜索联系人",
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
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ContactsPanelAction::ActivateSearch);
        });
        chat_search_pill(
            input,
            chat_sidebar_search_bg(),
            theme::border(),
            7.0,
            10.0,
            999.0,
        )
    }

    fn add_form_overlay(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::body("添加联系人".to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(self.add_field("名称", &self.add_name, 0, ContactsPanelAction::AddNameEdit));
        col.add_child(self.add_field(
            "Wormhole ID",
            &self.add_id,
            1,
            ContactsPanelAction::AddIdEdit,
        ));
        col.add_child(self.add_field(
            "邮箱（可选）",
            &self.add_email,
            2,
            ContactsPanelAction::AddEmailEdit,
        ));
        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        actions.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("取消".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::CloseAdd);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("保存".to_string(), self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_uniform_padding(10.0)
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ContactsPanelAction::SubmitAdd);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        col.add_child(actions.finish());

        Align::new(
            Container::new(col.finish())
                .with_background(theme::panel())
                .with_uniform_padding(18.0)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .finish(),
        )
        .finish()
    }

    fn add_field(
        &self,
        label: &str,
        value: &str,
        focus_idx: u8,
        _edit: fn(TextFieldEditAction) -> ContactsPanelAction,
    ) -> Box<dyn Element> {
        let focused = self.add_focused == focus_idx;
        let field_state = match focus_idx {
            0 => &self.add_name_field,
            1 => &self.add_id_field,
            _ => &self.add_email_field,
        };
        let action_ctor: fn(TextFieldEditAction) -> ContactsPanelAction = match focus_idx {
            0 => ContactsPanelAction::AddNameEdit,
            1 => ContactsPanelAction::AddIdEdit,
            _ => ContactsPanelAction::AddEmailEdit,
        };
        let field = render_search_field_with_caret(
            value,
            &field_state.marked_text,
            label,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
            field_state.cursor,
        );
        let input = TextFieldInput::builder(field, move |ctx, action| {
            ctx.dispatch_typed_action(action_ctor(action));
        })
        .focused(focused)
        .finish();
        Container::new(
            Flex::column()
                .with_child(
                    ui_text::chat_preview(label.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_child(input)
                .finish(),
        )
        .with_padding_top(10.0)
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
                ctx.notify();
            }
            ContactsPanelAction::FocusSearch | ContactsPanelAction::ActivateSearch => {
                self.search_focused = true;
                self.add_focused = 255;
                ctx.notify();
            }
            ContactsPanelAction::Select(id) => self.open_contact(id, ctx),
            ContactsPanelAction::OpenAdd => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.contacts_add_open = true;
                }
                self.add_focused = 0;
                ctx.notify();
            }
            ContactsPanelAction::CloseAdd => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.contacts_add_open = false;
                }
                ctx.notify();
            }
            ContactsPanelAction::AddNameEdit(edit) => {
                self.add_focused = 0;
                self.add_name_field.apply(&mut self.add_name, edit);
                ctx.notify();
            }
            ContactsPanelAction::AddIdEdit(edit) => {
                self.add_focused = 1;
                self.add_id_field.apply(&mut self.add_id, edit);
                ctx.notify();
            }
            ContactsPanelAction::AddEmailEdit(edit) => {
                self.add_focused = 2;
                self.add_email_field.apply(&mut self.add_email, edit);
                ctx.notify();
            }
            ContactsPanelAction::SubmitAdd => self.submit_add(ctx),
            ContactsPanelAction::Refresh => self.reload(ctx),
        }
    }
}
