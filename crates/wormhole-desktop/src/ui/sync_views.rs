use pathfinder_geometry::vector::vec2f;
use warpui::elements::{Container, Flex, ParentElement, Scrollable, ScrollableElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sync_commands::{list_sync_queue, sync_status};

pub struct SyncView {
    core: CoreHandle,
    font: FamilyId,
    status: String,
    queue: Vec<String>,
}

impl SyncView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            status: "加载同步状态…".into(),
            queue: Vec::new(),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = sync_status(&state).await;
                let queue = list_sync_queue(&state).await;
                (status, queue)
            },
            |view, output, ctx| {
                let (status, queue) = output;
                view.status = match status {
                    Ok(s) => format!(
                        "运行: {} 根: {} 队列 pending={} failed={}",
                        s.running, s.root_path, s.queue_pending, s.queue_failed
                    ),
                    Err(e) => format!("同步错误: {e}"),
                };
                view.queue = queue
                    .unwrap_or_default()
                    .into_iter()
                    .take(20)
                    .map(|item| {
                        format!(
                            "{} {} -> {:?}",
                            item.action,
                            item.source_path.as_deref().unwrap_or("—"),
                            item.status
                        )
                    })
                    .collect();
                ctx.notify();
            },
        );
    }
}

impl Entity for SyncView {
    type Event = ();
}

impl View for SyncView {
    fn ui_name() -> &'static str {
        "SyncView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(ui_text::title("同步", self.font).finish());
        col.add_child(ui_text::body(self.status.clone(), self.font).finish());
        for line in &self.queue {
            col.add_child(ui_text::mono(line.clone(), self.font).finish());
        }
        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}
