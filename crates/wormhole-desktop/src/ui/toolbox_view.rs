use std::sync::{Arc, Mutex};

use warpui::elements::{Container, DispatchEventResult, EventHandler, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{section_hint, SECTION_GAP};
use crate::ui::theme;
use crate::ui_text;
use crate::wormhole_native_ipc::host_control_window_key;

#[derive(Debug, Clone, Copy)]
struct ToolboxEntry {
    label: &'static str,
    enabled: bool,
}

#[derive(Debug, Clone)]
pub enum ToolboxAction {
    OpenRdpHost,
}

pub struct ToolboxView {
    #[allow(dead_code)]
    core: CoreHandle,
    coordinator: Arc<Mutex<CoordinatorState>>,
    font: FamilyId,
    tools: Vec<ToolboxEntry>,
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
                ToolboxEntry {
                    label: "RDP Host 控制台",
                    enabled: true,
                },
                ToolboxEntry {
                    label: "剪贴板历史",
                    enabled: false,
                },
                ToolboxEntry {
                    label: "Graphite",
                    enabled: false,
                },
                ToolboxEntry {
                    label: "HEVC 工具",
                    enabled: false,
                },
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

    fn tool_row(&self, entry: ToolboxEntry) -> Box<dyn Element> {
        let display = if entry.enabled {
            entry.label.to_string()
        } else {
            format!("{}（即将推出）", entry.label)
        };
        let text_color = if entry.enabled {
            theme::text()
        } else {
            theme::muted()
        };
        let background = if entry.enabled {
            theme::panel()
        } else {
            theme::bg()
        };
        let label_el = ui_text::body(display, self.font)
            .with_color(text_color)
            .finish();
        let interactive = if entry.enabled {
            EventHandler::new(label_el)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(ToolboxAction::OpenRdpHost);
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            label_el
        };
        Container::new(interactive)
            .with_background(background)
            .with_uniform_padding(12.0)
            .finish()
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
        grid.add_child(section_hint("可用工具可直接打开；灰色项正在开发中。", self.font));
        for entry in &self.tools {
            grid.add_child(self.tool_row(*entry));
            grid.add_child(
                Container::new(Flex::column().finish())
                    .with_vertical_margin(SECTION_GAP / 2.0)
                    .finish(),
            );
        }
        Container::new(grid.finish())
            .with_uniform_padding(8.0)
            .finish()
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
