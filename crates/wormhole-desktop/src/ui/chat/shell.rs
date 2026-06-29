use std::sync::{Arc, Mutex};

use warpui::elements::{
    ChildView, ConstrainedBox, Container, Expanded, Flex, MainAxisSize, ParentElement,
    Shrinkable,
};
use warpui::{AppContext, Element, Entity, View, ViewContext, ViewHandle};

use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::sidebar::ChatSidebarView;
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;

pub const SIDEBAR_WIDTH: f32 = 200.0;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

pub struct ChatShellView {
    selection: ConversationSelection,
    sidebar: ViewHandle<ChatSidebarView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
}

impl ChatShellView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let sidebar = ctx.add_typed_action_view(|ctx| {
            ChatSidebarView::new(ctx, core.clone(), selection.clone())
        });
        let thread = ctx.add_view(|ctx| ChatThreadView::new(ctx, core.clone(), selection.clone()));
        let compose = ctx.add_typed_action_view(|ctx| {
            ChatComposeView::new(ctx, core.clone(), selection.clone())
        });
        Self {
            selection,
            sidebar,
            thread,
            compose,
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
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ConstrainedBox::new(ChildView::new(&self.sidebar).finish())
                    .with_width(SIDEBAR_WIDTH)
                    .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.0,
                    Container::new(
                        Flex::column()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_child(
                                Expanded::new(1.0, ChildView::new(&self.thread).finish()).finish(),
                            )
                            .with_child(
                                Container::new(ChildView::new(&self.compose).finish())
                                    .finish(),
                            )
                            .finish(),
                    )
                    .with_background(theme::canvas())
                    .finish(),
                )
                .finish(),
            )
            .finish()
    }
}
