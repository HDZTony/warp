use std::sync::{Arc, Mutex};

use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::bubble::format_message_time_pub;
use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::online_dot;
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_conversations, ChatConversationDto,
};
use wormhole_desktop_core::cluster_commands::cluster_status;

pub const TG_SIDEBAR_AVATAR: f32 = 46.0;

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
    font: FamilyId,
    rows: Vec<SidebarRow>,
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
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            selection,
            font,
            rows: Vec::new(),
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
                (cfg, list, cluster)
            },
            |view, output, ctx| {
                let (cfg, list, cluster) = output;
                if let Ok(c) = cfg {
                    view.status = c.display_name;
                }
                let conversations = list.unwrap_or_default();
                view.rows = view.build_rows(conversations, cluster.ok());
                ctx.notify();
            },
        );
    }

    fn build_rows(
        &self,
        conversations: Vec<ChatConversationDto>,
        cluster: Option<wormhole_desktop_core::cluster_commands::ClusterStatusDto>,
    ) -> Vec<SidebarRow> {
        let mut rows = Vec::new();
        for conv in conversations {
            let title = conv
                .title
                .or(conv.peer_display_name.clone())
                .unwrap_or_else(|| conv.peer_endpoint.clone());
            let preview = conv.peer_endpoint.clone();
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
                for node in cluster.nodes {
                    let title = format!("{} · {}", node.os, node.hostname);
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
        let compact: String = title.chars().filter(|c| !c.is_whitespace()).take(2).collect();
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
            .with_main_axis_size(MainAxisSize::Max);
        top.add_child(
            Expanded::new(
                1.0,
                ui_text::body(row.title.clone(), self.font)
                    .with_color(if selected {
                        theme::text()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .finish(),
        );
        if !row.time.is_empty() {
            top.add_child(
                ui_text::device_meta(row.time.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        let mut preview_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        preview_row.add_child(
            Expanded::new(
                1.0,
                ui_text::body(row.preview.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
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
                .with_margin_top(4.0)
                .finish(),
        );

        let row_body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(
                    ConstrainedBox::new(
                        Container::new(
                            Align::new(
                                ui_text::body(Self::avatar_initials(&row.title), self.font)
                                    .with_color(theme::text())
                                    .finish(),
                            )
                            .finish(),
                        )
                        .with_background(theme::panel_elevated())
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
                            TG_SIDEBAR_AVATAR / 2.0,
                        )))
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .finish(),
                    )
                    .with_width(TG_SIDEBAR_AVATAR)
                    .with_height(TG_SIDEBAR_AVATAR)
                    .finish(),
                )
                .with_horizontal_margin(10.0)
                .finish(),
            )
            .with_child(Expanded::new(1.0, col.finish()).finish())
            .finish();

        let interactive = EventHandler::new(row_body)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::Select(id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish();

        Container::new(interactive)
            .with_padding_left(8.0)
            .with_padding_right(8.0)
            .with_padding_top(6.0)
            .with_padding_bottom(6.0)
            .with_background(if selected {
                theme::accent_cool_bg(32)
            } else {
                pathfinder_color::ColorU::transparent_black()
            })
            .finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let search_focused = self.search_focused;
        let draft = self.search.clone();
        let marked = self.search_field.marked_text.clone();
        let field = render_field_with_caret(
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
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "tab" || keystroke.key == "escape" {
                ctx.dispatch_typed_action(ChatSidebarAction::FocusSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ChatSidebarAction::ActivateSearch);
        })
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
                let active = selected.as_deref() == Some(row.id.as_str());
                list.add_child(self.chat_item(row, active));
            }
        }

        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Container::new(self.search_box())
                .with_horizontal_padding(12.0)
                .with_vertical_padding(10.0)
                .with_background(theme::panel())
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(
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
                if let Ok(mut guard) = self.selection.lock() {
                    *guard = Some(id.clone());
                }
                ctx.emit(ChatSidebarEvent::Selected(id.clone()));
                ctx.notify();
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

impl CaretBlinkHost for ChatSidebarView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
    }
}
