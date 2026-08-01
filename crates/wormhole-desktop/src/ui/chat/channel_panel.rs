//! New-channel modal — design §7.4.1 / desktop-current.html `.chat-channel-panel`.
//!
//! Creates a cluster group conversation (`chat_create_cluster_group`) as the
//! current channel carrier until a dedicated channel protocol lands.

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{StatusTone, HUD_RADIUS};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_create_cluster_group, CreateClusterGroupParams};
use wormhole_desktop_core::cluster_commands::cluster_status_hud;

const DIALOG_WIDTH: f32 = 410.0;
const AVATAR_SIZE: f32 = 72.0;
const NAME_MAX: usize = 48;
const DESC_MAX: usize = 120;
/// HTML `--void` on accent primary buttons.
fn void_ink() -> ColorU {
    ColorU::new(8, 7, 11, 255)
}

#[derive(Debug, Clone)]
pub enum ChannelPanelAction {
    Close,
    PickAvatar,
    ClearAvatar,
    NameEdit(TextFieldEditAction),
    DescEdit(TextFieldEditAction),
    ActivateName,
    ActivateDesc,
    Create,
}

#[derive(Debug, Clone)]
pub enum ChannelPanelEvent {
    Created(String),
    Closed,
}

pub struct ChannelPanelView {
    core: CoreHandle,
    shell_state: SharedChatShellState,
    font: FamilyId,
    name: String,
    name_field: TextFieldState,
    desc: String,
    desc_field: TextFieldState,
    /// 0 = none, 1 = name, 2 = description
    focused: u8,
    caret_blink: CaretBlink,
    avatar_path: Option<String>,
    feedback: String,
    creating: bool,
    last_open: bool,
}

impl ChannelPanelView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            shell_state,
            font,
            name: String::new(),
            name_field: TextFieldState::new(),
            desc: String::new(),
            desc_field: TextFieldState::new(),
            focused: 0,
            caret_blink: CaretBlink::new(),
            avatar_path: None,
            feedback: String::new(),
            creating: false,
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
                    .map(|s| s.channel_open)
                    .unwrap_or(false);
                if open && !view.last_open {
                    view.reset_form();
                    view.focused = 1;
                    view.last_open = true;
                    sync_caret_blink(view, ctx);
                    ctx.notify();
                } else if !open && view.last_open {
                    view.last_open = false;
                    view.creating = false;
                    ctx.notify();
                }
                view.poll_open(ctx);
            },
        );
    }

    fn reset_form(&mut self) {
        self.name.clear();
        self.name_field = TextFieldState::new();
        self.desc.clear();
        self.desc_field = TextFieldState::new();
        self.avatar_path = None;
        self.feedback.clear();
        self.creating = false;
        self.focused = 0;
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut state) = self.shell_state.lock() {
            state.close_channel();
        }
        self.reset_form();
        ctx.emit(ChannelPanelEvent::Closed);
        ctx.notify();
    }

    fn name_ok(&self) -> bool {
        !self.name.trim().is_empty() && !self.creating
    }

    fn submit(&mut self, ctx: &mut ViewContext<Self>) {
        let title = self.name.trim().to_string();
        if title.is_empty() {
            self.feedback = "请输入频道名称".into();
            ctx.notify();
            return;
        }
        if self.creating {
            return;
        }
        self.creating = true;
        self.feedback.clear();
        let core = self.core.clone();
        // Description/avatar are UI-only until a dedicated channel protocol exists.
        let _description = self.desc.trim().to_string();
        let _avatar = self.avatar_path.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let app = runtime.ctx.as_ref();
                let cluster = cluster_status_hud(&runtime.state).await?;
                let cluster_id = cluster
                    .cluster_id
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| "请先加入家庭集群".to_string())?;
                chat_create_cluster_group(
                    app,
                    &runtime.state,
                    CreateClusterGroupParams {
                        title,
                        cluster_id,
                        members: Vec::new(),
                        default_group: false,
                    },
                )
                .await
            },
            |view, result, ctx| {
                view.creating = false;
                match result {
                    Ok(conv) => {
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.close_channel();
                            state.show_toast("已创建频道", StatusTone::Success);
                        }
                        view.reset_form();
                        ctx.emit(ChannelPanelEvent::Created(conv.id));
                        ctx.notify();
                    }
                    Err(err) => {
                        view.feedback = format!("创建失败: {err}");
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.show_toast(format!("创建频道失败: {err}"), StatusTone::Danger);
                        }
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn text_field(&self, is_name: bool) -> Box<dyn Element> {
        let (draft, marked, placeholder, focused, cursor) = if is_name {
            (
                self.name.as_str(),
                self.name_field.marked_text.as_str(),
                "输入频道名称",
                self.focused == 1,
                self.name_field.cursor,
            )
        } else {
            (
                self.desc.as_str(),
                self.desc_field.marked_text.as_str(),
                "简单介绍这个频道",
                self.focused == 2,
                self.desc_field.cursor,
            )
        };
        let field = render_search_field_with_caret(
            draft,
            marked,
            placeholder,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
            cursor,
        );
        let input = if is_name {
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(ChannelPanelAction::NameEdit(action));
            })
            .focused(focused)
            .ime_preedit(!marked.is_empty())
            .finish()
        } else {
            TextFieldInput::builder(field, |ctx, action| {
                ctx.dispatch_typed_action(ChannelPanelAction::DescEdit(action));
            })
            .focused(focused)
            .ime_preedit(!marked.is_empty())
            .finish()
        };
        let input = if is_name {
            wrap_text_field_focus_on_click(input, |ctx| {
                ctx.dispatch_typed_action(ChannelPanelAction::ActivateName);
            })
        } else {
            wrap_text_field_focus_on_click(input, |ctx| {
                ctx.dispatch_typed_action(ChannelPanelAction::ActivateDesc);
            })
        };
        let border = if focused {
            theme::border_bright()
        } else {
            theme::border()
        };
        Container::new(
            Container::new(input)
                .with_padding_left(10.0)
                .with_padding_right(10.0)
                .with_padding_top(11.0)
                .with_padding_bottom(11.0)
                .finish(),
        )
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .finish()
    }

    fn labeled_field(&self, label: &str, input: Box<dyn Element>) -> Box<dyn Element> {
        Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Container::new(
                    ui_text::chat_preview(label.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_bottom(7.0)
                .finish(),
            )
            .with_child(input)
            .finish()
    }

    fn dialog_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        let mut head = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        head.add_child(
            Expanded::new(
                1.0,
                ui_text::chat_header_title("新建频道".to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        );
        head.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("×".to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(12.0)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChannelPanelAction::Close);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        col.add_child(
            Container::new(head.finish())
                .with_padding_left(18.0)
                .with_padding_right(10.0)
                .with_padding_top(12.0)
                .with_padding_bottom(12.0)
                .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                .finish(),
        );

        let mut form = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        let avatar_label = self
            .name
            .trim()
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "+".into());
        let avatar_inner = if self.avatar_path.is_some() {
            Align::new(
                ui_text::chat_preview("已选".to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish()
        } else {
            Align::new(
                ui_text::chat_avatar_glyph(avatar_label, self.font, 22.0)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish()
        };
        let avatar_btn = EventHandler::new(
            ConstrainedBox::new(
                Container::new(avatar_inner)
                    .with_background(theme::canvas())
                    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
                    .finish(),
            )
            .with_width(AVATAR_SIZE)
            .with_height(AVATAR_SIZE)
            .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChannelPanelAction::PickAvatar);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let mut avatar_copy = Flex::column().with_main_axis_size(MainAxisSize::Min);
        avatar_copy.add_child(
            ui_text::chat_preview("频道头像".to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        avatar_copy.add_child(
            Container::new(
                ui_text::device_meta("可选，支持常见图片格式".to_string(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(4.0)
            .finish(),
        );
        if self.avatar_path.is_some() {
            avatar_copy.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::device_meta("移除".to_string(), self.font)
                            .with_color(theme::accent_cool())
                            .finish(),
                    )
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChannelPanelAction::ClearAvatar);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_margin_top(6.0)
                .finish(),
            );
        }

        form.add_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(avatar_btn)
                .with_child(
                    Container::new(avatar_copy.finish())
                        .with_margin_left(14.0)
                        .finish(),
                )
                .finish(),
        );

        form.add_child(
            Container::new(self.labeled_field("频道名称", self.text_field(true)))
                .with_margin_top(16.0)
                .finish(),
        );
        form.add_child(
            Container::new(self.labeled_field("简介（可选）", self.text_field(false)))
                .with_margin_top(16.0)
                .finish(),
        );

        form.add_child(
            Container::new(
                ui_text::device_meta(self.feedback.clone(), self.font)
                    .with_color(theme::danger())
                    .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );

        let can_create = self.name_ok();
        let create_label = if self.creating {
            "创建中…".to_string()
        } else {
            "创建".to_string()
        };
        let mut actions = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        actions.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("取消".to_string(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(12.0)
                .with_padding_bottom(12.0)
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChannelPanelAction::Close);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        let create_btn = Container::new(
            ui_text::body(create_label, self.font)
                .with_color(if can_create {
                    void_ink()
                } else {
                    theme::muted()
                })
                .finish(),
        )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(12.0)
        .with_padding_bottom(12.0)
        .with_background(if can_create {
            theme::accent_cool()
        } else {
            theme::accent_cool_bg(20)
        })
        .with_border(Border::all(1.0).with_border_fill(theme::accent_cool()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .finish();
        actions.add_child(
            Container::new(
                EventHandler::new(create_btn)
                    .on_left_mouse_down(move |ctx, _, _| {
                        if can_create {
                            ctx.dispatch_typed_action(ChannelPanelAction::Create);
                        }
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_margin_left(7.0)
            .finish(),
        );
        form.add_child(
            Container::new(actions.finish())
                .with_margin_top(12.0)
                .finish(),
        );

        col.add_child(
            Container::new(form.finish())
                .with_padding_left(18.0)
                .with_padding_right(18.0)
                .with_padding_top(20.0)
                .with_padding_bottom(18.0)
                .finish(),
        );

        Container::new(col.finish())
            .with_background(theme::panel())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 2.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
            .finish()
    }
}

impl Entity for ChannelPanelView {
    type Event = ChannelPanelEvent;
}

impl View for ChannelPanelView {
    fn ui_name() -> &'static str {
        "ChannelPanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|s| s.channel_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }

        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 148))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChannelPanelAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let dialog = ConstrainedBox::new(self.dialog_body())
            .with_width(DIALOG_WIDTH)
            .finish();
        let dialog = EventHandler::new(dialog)
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish();

        Stack::new()
            .with_child(scrim)
            .with_child(Align::new(dialog).finish())
            .finish()
    }
}

impl TypedActionView for ChannelPanelView {
    type Action = ChannelPanelAction;

    fn handle_action(&mut self, action: &ChannelPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChannelPanelAction::Close => self.close(ctx),
            ChannelPanelAction::PickAvatar => {
                let picked = rfd::FileDialog::new()
                    .set_title("选择频道头像")
                    .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif"])
                    .pick_file();
                if let Some(path) = picked {
                    self.avatar_path = Some(path.display().to_string());
                }
                ctx.notify();
            }
            ChannelPanelAction::ClearAvatar => {
                self.avatar_path = None;
                ctx.notify();
            }
            ChannelPanelAction::NameEdit(edit) => {
                self.name_field.apply(&mut self.name, edit);
                if self.name.chars().count() > NAME_MAX {
                    self.name = self.name.chars().take(NAME_MAX).collect();
                }
                self.focused = 1;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChannelPanelAction::DescEdit(edit) => {
                self.desc_field.apply(&mut self.desc, edit);
                if self.desc.chars().count() > DESC_MAX {
                    self.desc = self.desc.chars().take(DESC_MAX).collect();
                }
                self.focused = 2;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChannelPanelAction::ActivateName => {
                self.focused = 1;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChannelPanelAction::ActivateDesc => {
                self.focused = 2;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ChannelPanelAction::Create => self.submit(ctx),
        }
    }
}

impl CaretBlinkHost for ChannelPanelView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.focused > 0
    }
}
