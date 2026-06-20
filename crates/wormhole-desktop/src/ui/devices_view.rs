use pathfinder_geometry::vector::vec2f;
use warpui::elements::{Container, Flex, ParentElement, Scrollable, ScrollableElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;

pub struct DevicesView {
    core: CoreHandle,
    font: FamilyId,
    summary: String,
}

impl DevicesView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            summary: "加载集群状态…".into(),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status(&state).await
            },
            |view, output, ctx| {
                view.summary = match output {
                    Ok(s) => format!(
                        "节点: {}\n集群: {}\n成员: {}\n存储卷: {}",
                        s.local_node_id,
                        s.cluster_id.as_deref().unwrap_or("未加入"),
                        s.nodes.len(),
                        s.storage_volumes.len()
                    ),
                    Err(e) => format!("集群错误: {e}"),
                };
                ctx.notify();
            },
        );
    }
}

impl Entity for DevicesView {
    type Event = ();
}

impl View for DevicesView {
    fn ui_name() -> &'static str {
        "DevicesView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let col = Flex::column()
            .with_child(ui_text::title("设备 / 集群", self.font).finish())
            .with_child(ui_text::mono(self.summary.clone(), self.font).finish());
        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}
