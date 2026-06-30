use std::sync::{Arc, Mutex};

use warpui::elements::{
    Border, ChildView, ConstrainedBox, Container, Expanded, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext, ViewHandle};

use crate::ui::chat::compose::ChatComposeView;
use crate::ui::chat::header::{ChatHeaderView, TG_HEADER_HEIGHT};
use crate::ui::chat::sidebar::ChatSidebarView;
use crate::ui::chat::thread::ChatThreadView;
use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{load_device_gate_status, wrap_with_device_gate, DeviceGateStatus};
use crate::ui::panel_primitives::tab_content_fill;
use crate::ui::theme;

pub const SIDEBAR_WIDTH: f32 = 300.0;

pub type ConversationSelection = Arc<Mutex<Option<String>>>;

pub struct ChatShellView {
    core: CoreHandle,
    font: FamilyId,
    gate: DeviceGateStatus,
    selection: ConversationSelection,
    sidebar: ViewHandle<ChatSidebarView>,
    header: ViewHandle<ChatHeaderView>,
    thread: ViewHandle<ChatThreadView>,
    compose: ViewHandle<ChatComposeView>,
}

impl ChatShellView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let selection = Arc::new(Mutex::new(None));
        let sidebar = ctx.add_typed_action_view(|ctx| {
            ChatSidebarView::new(ctx, core.clone(), selection.clone())
        });
        let header = ctx.add_view(|ctx| ChatHeaderView::new(ctx, core.clone(), selection.clone()));
        let thread = ctx.add_view(|ctx| ChatThreadView::new(ctx, core.clone(), selection.clone()));
        let compose = ctx.add_typed_action_view(|ctx| {
            ChatComposeView::new(ctx, core.clone(), selection.clone())
        });
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core: core.clone(),
            font,
            gate: DeviceGateStatus::default(),
            selection,
            sidebar,
            header,
            thread,
            compose,
        };
        view.poll_gate(ctx);
        view
    }

    fn poll_gate(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move { load_device_gate_status(&core.runtime().state).await },
            |view, gate, ctx| {
                view.gate = gate;
                ctx.notify();
            },
        );
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
        wrap_with_device_gate(self.font, "P2P 聊天", &self.gate, self.chat_body())
    }
}

impl ChatShellView {
    fn chat_body(&self) -> Box<dyn Element> {
        tab_content_fill(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    ConstrainedBox::new(ChildView::new(&self.sidebar).finish())
                        .with_width(SIDEBAR_WIDTH)
                        .finish(),
                )
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(
                            Flex::column()
                                .with_main_axis_size(MainAxisSize::Max)
                                .with_child(
                                    ConstrainedBox::new(ChildView::new(&self.header).finish())
                                        .with_height(TG_HEADER_HEIGHT)
                                        .finish(),
                                )
                                .with_child(
                                    Expanded::new(1.0, ChildView::new(&self.thread).finish())
                                        .finish(),
                                )
                                .with_child(ChildView::new(&self.compose).finish())
                                .finish(),
                        )
                        .with_background(theme::canvas())
                        .with_border(Border::left(1.0).with_border_fill(theme::border()))
                        .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
    }
}
