use warpui::elements::{Container, Flex, ParentElement, Scrollable, ScrollableElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};
use pathfinder_geometry::vector::vec2f;

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::remote_agent_commands::remote_agent_list_tasks;

pub struct AgentThreadsSidebar {
    core: CoreHandle,
    font: FamilyId,
    lines: Vec<String>,
}

impl AgentThreadsSidebar {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            lines: vec!["Agent 任务".into()],
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                remote_agent_list_tasks(&state, None).await
            },
            |view, output, ctx| {
                match output {
                    Ok(tasks) => {
                        view.lines = tasks
                            .into_iter()
                            .map(|t| {
                                format!(
                                    "{} [{:?}] {}",
                                    t.task_id,
                                    t.status,
                                    t.message.as_deref().unwrap_or("—")
                                )
                            })
                            .collect();
                    }
                    Err(e) => view.lines = vec![format!("Agent 错误: {e}")],
                }
                ctx.notify();
            },
        );
    }
}

impl Entity for AgentThreadsSidebar {
    type Event = ();
}

impl View for AgentThreadsSidebar {
    fn ui_name() -> &'static str {
        "AgentThreadsSidebar"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(ui_text::title("Agent", self.font).finish());
        for line in &self.lines {
            col.add_child(ui_text::mono(line.clone(), self.font).finish());
        }
        Container::new(col.finish())
                    .with_background(theme::canvas())
                    .with_uniform_padding(8.0).finish()
    }
}
