use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{section_hint, SECTION_GAP};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy)]
struct ToolboxEntry {
    label: &'static str,
    enabled: bool,
}

#[derive(Debug, Clone)]
pub enum ToolboxAction {}

pub struct ToolboxView {
    #[allow(dead_code)]
    core: CoreHandle,
    #[allow(dead_code)]
    coordinator: std::sync::Arc<std::sync::Mutex<crate::coordinator::CoordinatorState>>,
    font: FamilyId,
    tools: Vec<ToolboxEntry>,
}

impl ToolboxView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: std::sync::Arc<std::sync::Mutex<crate::coordinator::CoordinatorState>>,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        Self {
            core,
            coordinator,
            font,
            tools: vec![
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

    fn tool_card(&self, entry: ToolboxEntry) -> Box<dyn Element> {
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
        Container::new(
            ui_text::body(display, self.font)
                .with_color(text_color)
                .finish(),
        )
        .with_uniform_padding(16.0)
        .with_background(theme::panel())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(crate::ui::panel_primitives::HUD_RADIUS)))
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
        let mut col = Flex::column();
        col.add_child(ui_text::title("工具箱", self.font).finish());
        col.add_child(section_hint("系统模块以网格展示；灰色项正在开发中。", self.font));

        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        for entry in &self.tools {
            row.add_child(
                Container::new(self.tool_card(*entry))
                    .with_uniform_margin(SECTION_GAP / 2.0)
                    .finish(),
            );
        }
        col.add_child(row.finish());

        Container::new(col.finish())
            .with_uniform_padding(8.0)
            .finish()
    }
}

impl TypedActionView for ToolboxView {
    type Action = ToolboxAction;

    fn handle_action(&mut self, _action: &ToolboxAction, _ctx: &mut ViewContext<Self>) {}
}
