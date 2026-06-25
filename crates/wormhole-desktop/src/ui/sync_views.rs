use warpui::elements::{Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, truncate_middle, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sync_commands::{list_sync_queue, sync_status};

pub struct SyncView {
    core: CoreHandle,
    font: FamilyId,
    status: String,
    status_error: bool,
    queue: Vec<String>,
    loading: bool,
}

impl SyncView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            status: "加载同步状态…".into(),
            status_error: false,
            queue: Vec::new(),
            loading: true,
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.loading = true;
        ctx.notify();
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
                view.loading = false;
                match status {
                    Ok(s) => {
                        view.status_error = false;
                        view.status = format!(
                            "运行: {} · 根目录: {} · 队列 pending={} · failed={}",
                            if s.running { "是" } else { "否" },
                            truncate_middle(&s.root_path, 48),
                            s.queue_pending,
                            s.queue_failed
                        );
                    }
                    Err(e) => {
                        view.status_error = true;
                        view.status = format!("同步错误: {}", truncate_middle(&e, 120));
                    }
                }
                view.queue = queue
                    .unwrap_or_default()
                    .into_iter()
                    .take(20)
                    .map(|item| {
                        format!(
                            "{} {} → {:?}",
                            item.action,
                            truncate_middle(
                                item.source_path.as_deref().unwrap_or("—"),
                                56,
                            ),
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
        col.add_child(section_title("同步队列", self.font));
        col.add_child(section_hint("最近 20 条队列项。失败项请在日志或设置中排查。", self.font));
        if self.loading {
            col.add_child(status_line("加载中…", self.font, StatusTone::Placeholder));
        } else if self.status_error {
            col.add_child(status_line(self.status.clone(), self.font, StatusTone::Danger));
        } else {
            col.add_child(status_line(self.status.clone(), self.font, StatusTone::Neutral));
        }
        if !self.loading && self.queue.is_empty() && !self.status_error {
            col.add_child(status_line(
                "队列为空。文件变更将出现在此处。",
                self.font,
                StatusTone::Placeholder,
            ));
        } else {
            for line in &self.queue {
                col.add_child(
                    ui_text::mono(line.clone(), self.font)
                        .with_color(theme::text())
                        .finish(),
                );
            }
        }
        section_card(col.finish())
    }
}
