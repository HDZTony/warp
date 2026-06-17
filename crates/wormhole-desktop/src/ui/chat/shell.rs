use std::sync::{Arc, Mutex};

use warpui::elements::{ChildView, Flex, ParentElement, Shrinkable};
use warpui::{AppContext, Element, Entity, View, ViewContext, ViewHandle};

use crate::ui::chat::agent_threads_sidebar::AgentThreadsSidebar;
use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::sidebar::ChatSidebarView;
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::core_handle::CoreHandle;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

pub struct ChatShellView {
    selection: ConversationSelection,
    sidebar: ViewHandle<ChatSidebarView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
    agents: ViewHandle<AgentThreadsSidebar>,
}

impl ChatShellView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let sidebar = ctx.add_view(|ctx| ChatSidebarView::new(ctx, core.clone(), selection.clone()));
        let thread = ctx.add_view(|ctx| ChatThreadView::new(ctx, core.clone(), selection.clone()));
        let compose = ctx.add_view(|ctx| ChatComposeView::new(ctx, core.clone(), selection.clone()));
        let agents = ctx.add_view(|ctx| AgentThreadsSidebar::new(ctx, core));
        Self {
            selection,
            sidebar,
            thread,
            compose,
            agents,
        }
    }
}

impl Entity for ChatShellView {
    type Event = ();
}

impl View for ChatShellView {
    fn ui_name() -> &'static str {
        "ChatShellView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Flex::row()
            .with_child(ChildView::new(&self.sidebar).finish())
            .with_child(
                Shrinkable::new(
                    1.0,
                    Flex::column()
                        .with_child(ChildView::new(&self.thread).finish())
                        .with_child(ChildView::new(&self.compose).finish())
                        .finish(),
                )
                .finish(),
            )
            .with_child(ChildView::new(&self.agents).finish())
            .finish()
    }
}
