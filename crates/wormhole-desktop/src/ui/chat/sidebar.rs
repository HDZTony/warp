use std::sync::{Arc, Mutex};

use pathfinder_geometry::vector::vec2f;
use warpui::elements::{
    Container, DispatchEventResult, EventHandler, Flex, ParentElement, Scrollable,
    ScrollableElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
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

pub struct ChatSidebarView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
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
            conversations: Vec::new(),
            status: "加载会话…".into(),
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
                (cfg, list)
            },
            |view, output, ctx| {
                let (cfg, list) = output;
                if let Ok(c) = cfg {
                    view.status = format!("{} ({})", c.display_name, c.endpoint_id);
                }
                view.conversations = match list {
                    Ok(conversations) => conversations,
                    Err(_) => Vec::new(),
                };
                ctx.notify();
            },
        );
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
        col.add_child(ui_text::title("会话", self.font).finish());
        col.add_child(ui_text::body(self.status.clone(), self.font).finish());
        for conv in &self.conversations {
            let title = conv
                .title
                .clone()
                .or(conv.peer_display_name.clone())
                .unwrap_or_else(|| conv.id.clone());
            let id = conv.id.clone();
            let active = selected.as_deref() == Some(id.as_str());
            let row = EventHandler::new(ui_text::body(title, self.font).finish())
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(ChatSidebarAction::Select(id.clone()));
                    DispatchEventResult::StopPropagation
                })
                .finish();
            col.add_child(
                Container::new(row)
                    .with_background(if active {
                        theme::accent_bg(48)
                    } else {
                        theme::panel()
                    })
                    .with_uniform_padding(8.0)
                    .finish(),
            );
        }
        Container::new(col.finish())
            .with_uniform_padding(8.0)
            .with_background(theme::canvas())
            .finish()
    }
}

impl TypedActionView for ChatSidebarView {
    type Action = ChatSidebarAction;

    fn handle_action(&mut self, action: &ChatSidebarAction, ctx: &mut ViewContext<Self>) {
        if let ChatSidebarAction::Select(id) = action {
            if let Ok(mut guard) = self.selection.lock() {
                *guard = Some(id.clone());
            }
            ctx.emit(ChatSidebarEvent::Selected(id.clone()));
            ctx.notify();
        }
    }
}
