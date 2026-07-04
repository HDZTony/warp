use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Flex,
    MainAxisAlignment, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::online_dot;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;

pub const TG_HEADER_HEIGHT: f32 = 56.0;
const TG_HEADER_AVATAR: f32 = 40.0;
const TG_HEADER_BTN: f32 = 36.0;
const TG_HEADER_PAD_X: f32 = 16.0;
const TG_HEADER_PAD_Y: f32 = 8.0;
const TG_HEADER_INFO_GAP: f32 = 10.0;
const TG_HEADER_ACTION_GAP: f32 = 2.0;

pub struct ChatHeaderView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    title: String,
    status: String,
    online: bool,
}

impl ChatHeaderView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            selection,
            font,
            title: "选择左侧终端".into(),
            status: String::new(),
            online: false,
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            },
            |view, _, ctx| {
                view.refresh_from_selection(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn refresh_from_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected = self.selection.lock().ok().and_then(|g| g.clone());
        if selected.is_none() {
            self.title = "选择左侧终端".into();
            self.status = "从列表中选择会话".into();
            self.online = false;
            ctx.notify();
            return;
        }
        let selected = selected.unwrap();
        self.title = selected.clone();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status(&state).await
            },
            |view, output, ctx| {
                let selected = view.selection.lock().ok().and_then(|g| g.clone());
                let Some(selected) = selected else {
                    return;
                };
                if let Ok(cluster) = output {
                    if let Some(node) = cluster.nodes.iter().find(|n| {
                        n.chat_endpoint_id.as_deref() == Some(selected.as_str())
                            || n.node_id == selected
                    }) {
                        view.title = format!("{} · {}", node.os, node.hostname);
                        view.online = node.online;
                        view.status = if node.online {
                            "在线".into()
                        } else {
                            "离线".into()
                        };
                    } else {
                        view.title = "未知设备".into();
                        view.status = "会话信息同步中".into();
                        view.online = false;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn avatar_initials(title: &str) -> String {
        let compact: String = title
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(2)
            .collect();
        if compact.is_empty() {
            "WH".to_string()
        } else {
            compact.to_uppercase()
        }
    }

    fn header_button(icon_path: &'static str) -> Box<dyn Element> {
        Container::new(Align::new(icons::chat_header_icon(icon_path, theme::muted())).finish())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(TG_HEADER_BTN / 2.0)))
            .finish()
    }
}

impl Entity for ChatHeaderView {
    type Event = ();
}

impl View for ChatHeaderView {
    fn ui_name() -> &'static str {
        "ChatHeaderView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut info = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        info.add_child(
            ConstrainedBox::new(
                Container::new(
                    Align::new(
                        ui_text::body(Self::avatar_initials(&self.title), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_background(theme::panel_elevated())
                .with_corner_radius(warpui::elements::CornerRadius::with_all(
                    warpui::elements::Radius::Pixels(TG_HEADER_AVATAR / 2.0),
                ))
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .finish(),
            )
            .with_width(TG_HEADER_AVATAR)
            .with_height(TG_HEADER_AVATAR)
            .finish(),
        );

        let mut text_col = Flex::column().with_main_axis_size(MainAxisSize::Min);
        text_col.add_child(
            ui_text::body(self.title.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        let status_color = if self.online {
            theme::success()
        } else {
            theme::muted()
        };
        text_col.add_child(
            Container::new(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(if self.online {
                        Container::new(online_dot())
                            .with_horizontal_margin(4.0)
                            .finish()
                    } else {
                        Flex::row().finish()
                    })
                    .with_child(
                        ui_text::body(self.status.clone(), self.font)
                            .with_color(status_color)
                            .finish(),
                    )
                    .finish(),
            )
            .with_margin_top(2.0)
            .finish(),
        );
        info.add_child(
            Container::new(text_col.finish())
                .with_margin_left(TG_HEADER_INFO_GAP)
                .finish(),
        );

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Min);
        for (index, icon_path) in [
            "chat-header-search.svg",
            "chat-header-phone.svg",
            "chat-header-more.svg",
        ]
        .into_iter()
        .enumerate()
        {
            actions.add_child(
                Container::new(
                    ConstrainedBox::new(Self::header_button(icon_path))
                        .with_width(TG_HEADER_BTN)
                        .with_height(TG_HEADER_BTN)
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

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(info.finish())
                .with_child(actions.finish())
                .finish(),
        )
        .with_padding_left(TG_HEADER_PAD_X)
        .with_padding_right(TG_HEADER_PAD_X)
        .with_padding_top(TG_HEADER_PAD_Y)
        .with_padding_bottom(TG_HEADER_PAD_Y)
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .with_background(theme::panel())
        .finish()
        .into()
    }
}
