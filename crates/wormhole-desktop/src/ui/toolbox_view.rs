use std::sync::{Arc, Mutex};

use warpui::elements::{
    Container, DispatchEventResult, EventHandler, Flex, ParentElement, Scrollable,
    ScrollableElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use pathfinder_geometry::vector::vec2f;

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_native_ipc::host_control_window_key;

#[derive(Debug, Clone)]
pub enum ToolboxAction {
    OpenRdpHost,
}

pub struct ToolboxView {
    core: CoreHandle,
    coordinator: Arc<Mutex<CoordinatorState>>,
    font: FamilyId,
    tools: Vec<&'static str>,
}

impl ToolboxView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: Arc<Mutex<CoordinatorState>>,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            core,
            coordinator,
            font,
            tools: vec![
                "RDP Host 控制台",
                "剪贴板历史",
                "Graphite",
                "HEVC 工具",
            ],
        }
    }

    fn open_rdp_host(&self, ctx: &mut ViewContext<Self>) {
        let window_key = host_control_window_key().to_string();
        if let Ok(mut guard) = self.coordinator.lock() {
            guard.enqueue(UiCommand::OpenHostControl {
                window_key: window_key.clone(),
                title: "Remote Desktop Host".into(),
            });
        }
        ctx.notify();
    }
}

impl Entity for ToolboxView {
    type Event = ();
}

impl View for ToolboxView {
    fn ui_name() -> &'static str {
        "ToolboxView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut grid = Flex::column();
        grid.add_child(ui_text::title("工具箱", self.font).finish());
        for tool in &self.tools {
            let label = if *tool == "RDP Host 控制台" {
                EventHandler::new(ui_text::body(*tool, self.font).finish())
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(ToolboxAction::OpenRdpHost);
                        DispatchEventResult::StopPropagation
                    })
                    .finish()
            } else {
                ui_text::body(*tool, self.font).finish()
            };
            grid.add_child(
                Container::new(label)
                    .with_background(theme::panel())
                    .with_uniform_padding(12.0)
                    .finish(),
            );
        }
        Container::new(grid.finish()).with_uniform_padding(8.0).finish()
    }
}

impl TypedActionView for ToolboxView {
    type Action = ToolboxAction;

    fn handle_action(&mut self, action: &ToolboxAction, ctx: &mut ViewContext<Self>) {
        match action {
            ToolboxAction::OpenRdpHost => self.open_rdp_host(ctx),
        }
    }
}
