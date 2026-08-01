//! Create-channel modal — design §7.4.1.
//!
//! Selected avatar must show a circular thumbnail preview (never text-only 「已选」).

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, Image, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::CacheOption;

use crate::ui::chat::image_asset::insert_attachment_image_asset;
use crate::ui::chat::shell_state::SharedChatShellState;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{tg_avatar, StatusTone, HUD_RADIUS};
use crate::ui::text_field_input::{
    render_search_field_with_caret, wrap_text_field_focus_on_click, CaretBlink, TextFieldEditAction,
    TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{chat_create_channel, CreateChatChannelParams};

const DIALOG_WIDTH: f32 = 420.0;

#[derive(Debug, Clone)]
pub enum ChannelCreateAction {
    Close,
    TitleEdit(TextFieldEditAction),
    DescEdit(TextFieldEditAction),
    FocusTitle,
    FocusDesc,
    PickAvatar,
    ClearAvatar,
    Submit,
}

#[derive(Debug, Clone)]
pub enum ChannelCreateEvent {
    Created(String),
    Closed,
}

pub struct ChannelCreatePanelView {
    core: CoreHandle,
    shell_state: SharedChatShellState,
    font: FamilyId,
    title: String,
    title_field: TextFieldState,
    title_focused: bool,
    description: String,
    desc_field: TextFieldState,
    desc_focused: bool,
    avatar_path: Option<String>,
    avatar_asset_id: Option<String>,
    status: String,
    status_tone: StatusTone,
    submitting: bool,
    caret_blink: CaretBlink,
}

impl ChannelCreatePanelView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        shell_state: SharedChatShellState,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            core,
            shell_state,
            font,
            title: String::new(),
            title_field: TextFieldState::new(),
            title_focused: true,
            description: String::new(),
            desc_field: TextFieldState::new(),
            desc_focused: false,
            avatar_path: None,
            avatar_asset_id: None,
            status: String::new(),
            status_tone: StatusTone::Muted,
            submitting: false,
            caret_blink: CaretBlink::new(),
        }
    }

    fn can_submit(&self) -> bool {
        !self.submitting && !self.title.trim().is_empty()
    }

    fn pick_avatar(&mut self, ctx: &mut ViewContext<Self>) {
        let path = rfd::FileDialog::new()
            .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
            .pick_file();
        let Some(path) = path else {
            return;
        };
        match insert_attachment_image_asset(ctx, "channel-avatar-preview", &path) {
            Ok(asset_id) => {
                self.avatar_path = Some(path.display().to_string());
                self.avatar_asset_id = Some(asset_id);
                self.status.clear();
            }
            Err(err) => {
                self.avatar_path = None;
                self.avatar_asset_id = None;
                self.status = format!("无法预览头像: {err}");
                self.status_tone = StatusTone::Danger;
            }
        }
        ctx.notify();
    }

    fn submit(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.can_submit() {
            return;
        }
        self.submitting = true;
        self.status.clear();
        let core = self.core.clone();
        let params = CreateChatChannelParams {
            title: self.title.trim().to_string(),
            description: {
                let desc = self.description.trim();
                if desc.is_empty() {
                    None
                } else {
                    Some(desc.to_string())
                }
            },
            avatar_source_path: self.avatar_path.clone(),
        };
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                chat_create_channel(&runtime.ctx, &runtime.state, params).await
            },
            |view, result, ctx| {
                view.submitting = false;
                match result {
                    Ok(conv) => {
                        view.title.clear();
                        view.description.clear();
                        view.avatar_path = None;
                        view.avatar_asset_id = None;
                        if let Ok(mut state) = view.shell_state.lock() {
                            state.channel_create_open = false;
                        }
                        ctx.emit(ChannelCreateEvent::Created(conv.id));
                        ctx.notify();
                    }
                    Err(err) => {
                        view.status = err;
                        view.status_tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
        ctx.notify();
    }

    fn avatar_block(&self) -> Box<dyn Element> {
        let preview: Box<dyn Element> = if let Some(asset_id) = &self.avatar_asset_id {
            ConstrainedBox::new(
                Container::new(
                    Image::new(
                        AssetSource::Raw {
                            id: asset_id.clone(),
                        },
                        CacheOption::BySize,
                    )
                    .finish(),
                )
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.0)))
                .finish(),
            )
            .with_width(64.0)
            .with_height(64.0)
            .finish()
        } else {
            let initials = {
                let trimmed = self.title.trim();
                if trimmed.is_empty() {
                    "频".into()
                } else {
                    trimmed.chars().take(2).collect::<String>()
                }
            };
            tg_avatar(initials, self.font, 64.0)
        };

        let mut col = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        col.add_child(
            ui_text::chat_preview("频道头像", self.font)
                .with_color(theme::muted())
                .finish(),
        );
        col.add_child(
            Container::new(preview)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .finish(),
        );
        col.add_child(
            EventHandler::new(
                ui_text::chat_preview("选择图片", self.font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChannelCreateAction::PickAvatar);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        if self.avatar_path.is_some() {
            col.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::chat_preview("移除", self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(ChannelCreateAction::ClearAvatar);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_padding_top(4.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(
                ui_text::chat_preview("可选，支持常见图片格式", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_padding_top(4.0)
            .finish(),
        );
        col.finish()
    }

    fn text_field(
        &self,
        value: &str,
        field: &TextFieldState,
        placeholder: &str,
        focused: bool,
        focus_action: ChannelCreateAction,
        edit_ctor: fn(TextFieldEditAction) -> ChannelCreateAction,
    ) -> Box<dyn Element> {
        let rendered = render_search_field_with_caret(
            value,
            &field.marked_text,
            placeholder,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
            field.cursor,
        );
        let input = TextFieldInput::builder(rendered, move |ctx, action| {
            ctx.dispatch_typed_action(edit_ctor(action));
        })
        .focused(focused)
        .finish();
        wrap_text_field_focus_on_click(
            Container::new(input)
                .with_padding_left(10.0)
                .with_padding_right(10.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .with_background(theme::panel_elevated())
                .with_border(Border::all(1.0).with_border_fill(if focused {
                    theme::accent_cool()
                } else {
                    theme::border()
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
            move |ctx| {
                ctx.dispatch_typed_action(focus_action.clone());
            },
        )
    }

    fn dialog_body(&self) -> Box<dyn Element> {
        let mut form = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        form.add_child(
            ui_text::body("新建频道", self.font)
                .with_color(theme::text())
                .finish(),
        );

        let mut body = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Start);
        body.add_child(self.avatar_block());
        let mut fields = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        fields.add_child(
            Container::new(
                ui_text::chat_preview("频道名称", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_padding_bottom(4.0)
            .finish(),
        );
        fields.add_child(self.text_field(
            &self.title,
            &self.title_field,
            "输入频道名称",
            self.title_focused,
            ChannelCreateAction::FocusTitle,
            ChannelCreateAction::TitleEdit,
        ));
        fields.add_child(
            Container::new(
                ui_text::chat_preview("简介（可选）", self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_padding_top(10.0)
            .with_padding_bottom(4.0)
            .finish(),
        );
        fields.add_child(self.text_field(
            &self.description,
            &self.desc_field,
            "简单介绍这个频道",
            self.desc_focused,
            ChannelCreateAction::FocusDesc,
            ChannelCreateAction::DescEdit,
        ));
        body.add_child(
            Container::new(Expanded::new(1.0, fields.finish()).finish())
                .with_padding_left(16.0)
                .finish(),
        );
        form.add_child(Container::new(body.finish()).with_padding_top(16.0).finish());

        if !self.status.is_empty() {
            let color = match self.status_tone {
                StatusTone::Danger => theme::danger(),
                StatusTone::Warn => theme::warn(),
                StatusTone::Success => theme::success(),
                StatusTone::Muted | StatusTone::Placeholder => theme::muted(),
                StatusTone::Neutral => theme::text(),
            };
            form.add_child(
                Container::new(
                    ui_text::chat_preview(self.status.clone(), self.font)
                        .with_color(color)
                        .finish(),
                )
                .with_padding_top(8.0)
                .finish(),
            );
        }

        let mut actions = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::End);
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body("取消", self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .finish(),
            )
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(ChannelCreateAction::Close);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        let can_submit = self.can_submit();
        let submit_bg = if can_submit {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let submit_fg = if can_submit {
            ColorU::white()
        } else {
            theme::muted()
        };
        actions.add_child(
            EventHandler::new(
                Container::new(
                    ui_text::body(if self.submitting { "创建中…" } else { "创建" }, self.font)
                        .with_color(submit_fg)
                        .finish(),
                )
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .with_background(submit_bg)
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                if can_submit {
                    ctx.dispatch_typed_action(ChannelCreateAction::Submit);
                }
                DispatchEventResult::StopPropagation
            })
            .finish(),
        );
        form.add_child(Container::new(actions.finish()).with_padding_top(16.0).finish());

        Container::new(
            ConstrainedBox::new(form.finish())
                .with_width(DIALOG_WIDTH)
                .finish(),
        )
        .with_padding_left(20.0)
        .with_padding_right(20.0)
        .with_padding_top(18.0)
        .with_padding_bottom(18.0)
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 2.0)))
        .finish()
    }
}

impl Entity for ChannelCreatePanelView {
    type Event = ChannelCreateEvent;
}

impl View for ChannelCreatePanelView {
    fn ui_name() -> &'static str {
        "ChannelCreatePanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let open = self
            .shell_state
            .lock()
            .map(|state| state.channel_create_open)
            .unwrap_or(false);
        if !open {
            return Flex::column().finish();
        }

        let scrim = EventHandler::new(
            Container::new(Flex::column().finish())
                .with_background(ColorU::new(0, 0, 0, 140))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(ChannelCreateAction::Close);
            DispatchEventResult::StopPropagation
        })
        .finish();

        let dialog = EventHandler::new(self.dialog_body())
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish();

        Stack::new()
            .with_child(scrim)
            .with_child(Align::new(dialog).finish())
            .finish()
    }
}

impl TypedActionView for ChannelCreatePanelView {
    type Action = ChannelCreateAction;

    fn handle_action(&mut self, action: &ChannelCreateAction, ctx: &mut ViewContext<Self>) {
        match action {
            ChannelCreateAction::Close => {
                if let Ok(mut state) = self.shell_state.lock() {
                    state.channel_create_open = false;
                }
                ctx.emit(ChannelCreateEvent::Closed);
                ctx.notify();
            }
            ChannelCreateAction::TitleEdit(edit) => {
                self.title_field.apply(&mut self.title, edit);
                self.title_focused = true;
                self.desc_focused = false;
                ctx.notify();
            }
            ChannelCreateAction::DescEdit(edit) => {
                self.desc_field.apply(&mut self.description, edit);
                self.title_focused = false;
                self.desc_focused = true;
                ctx.notify();
            }
            ChannelCreateAction::FocusTitle => {
                self.title_focused = true;
                self.desc_focused = false;
                ctx.notify();
            }
            ChannelCreateAction::FocusDesc => {
                self.title_focused = false;
                self.desc_focused = true;
                ctx.notify();
            }
            ChannelCreateAction::PickAvatar => self.pick_avatar(ctx),
            ChannelCreateAction::ClearAvatar => {
                self.avatar_path = None;
                self.avatar_asset_id = None;
                ctx.notify();
            }
            ChannelCreateAction::Submit => self.submit(ctx),
        }
    }
}
