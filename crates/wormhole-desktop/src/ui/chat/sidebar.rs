use std::sync::{Arc, Mutex};

use warpui::elements::{
    Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{section_hint, section_title, status_line, StatusTone};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;
use wormhole_desktop_core::chat_commands::{
    chat_config, chat_list_conversations, ChatConversationDto,
};

#[derive(Debug, Clone)]
pub enum ChatSidebarAction {
    Select(String),
}

#[derive(Debug, Clone)]
pub enum ChatSidebarEvent {
    Selected(String),
}

struct TerminalRow {
    id: String,
    title: String,
    subtitle: String,
    online: bool,
}

pub struct ChatSidebarView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    terminals: Vec<TerminalRow>,
    conversations: Vec<ChatConversationDto>,
    status: String,
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
            terminals: Vec::new(),
            conversations: Vec::new(),
            status: "加载终端…".into(),
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
                    view.status = format!("本机 {}", c.display_name);
                }
                view.conversations = match list {
                    Ok(conversations) => conversations,
                    Err(_) => Vec::new(),
                };
                view.terminals = match cluster {
                    Ok(status) => status
                        .nodes
                        .into_iter()
                        .map(|node| {
                            let title = format!("{} · {}", node.os, node.hostname);
                            TerminalRow {
                                id: node
                                    .chat_endpoint_id
                                    .clone()
                                    .unwrap_or_else(|| node.node_id.clone()),
                                title,
                                subtitle: node.node_id,
                                online: node.online,
                            }
                        })
                        .collect(),
                    Err(e) => {
                        view.status = format!("集群错误: {e}");
                        Vec::new()
                    }
                };
                ctx.notify();
            },
        );
    }

    fn terminal_row(&self, terminal: &TerminalRow, selected: bool) -> Box<dyn Element> {
        let id = terminal.id.clone();
        let mut title_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center);
        title_row.add_child(
            ui_text::body(terminal.title.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        let status_label = if terminal.online { "在线" } else { "离线" };
        title_row.add_child(
            Container::new(
                ui_text::body(status_label, self.font)
                    .with_color(if terminal.online {
                        theme::success()
                    } else {
                        theme::muted()
                    })
                    .finish(),
            )
            .with_horizontal_margin(8.0)
            .finish(),
        );

        let mut col = Flex::column();
        col.add_child(title_row.finish());
        col.add_child(
            ui_text::mono(terminal.subtitle.clone(), self.font)
                .with_color(theme::muted())
                .finish(),
        );

        let row = EventHandler::new(col.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(ChatSidebarAction::Select(id.clone()));
                DispatchEventResult::StopPropagation
            })
            .finish();

        Container::new(row)
            .with_background(if selected {
                theme::accent_bg(32)
            } else {
                theme::panel()
            })
            .with_uniform_padding(10.0)
            .finish()
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
        let mut col = Flex::column();
        col.add_child(section_title("终端", self.font));
        let node_count = self.terminals.len();
        col.add_child(section_hint(
            format!("CLUSTER · {node_count} NODES · E2E"),
            self.font,
        ));

        if self.terminals.is_empty() {
            col.add_child(status_line(
                if self.status.is_empty() {
                    "暂无集群终端".to_string()
                } else {
                    self.status.clone()
                },
                self.font,
                StatusTone::Placeholder,
            ));
        } else {
            for terminal in &self.terminals {
                let active = selected.as_deref() == Some(terminal.id.as_str());
                col.add_child(self.terminal_row(terminal, active));
            }
        }

        Container::new(col.finish())
            .with_uniform_padding(crate::ui::panel_primitives::SECTION_PADDING)
            .with_background(theme::canvas())
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
        }
    }
}
