use warpui::elements::{
    Border, Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, truncate_middle, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sync_commands::{
    list_sync_queue, search_sync_entries, sync_status, SearchSyncParams, SyncEntryDto,
};
use wormhole_desktop_core::workspace_ui::{
    workspace_list_workers, workspace_open_file, workspace_start_session_worker, WorkspaceMode,
    WorkspaceOpenRequest,
};

#[derive(Debug, Clone)]
pub enum SyncAction {
    Refresh,
    OpenWithPaint(String),
}

pub struct SyncView {
    core: CoreHandle,
    font: FamilyId,
    status: String,
    status_error: bool,
    queue: Vec<String>,
    loading: bool,
    remote_files: Vec<SyncEntryDto>,
    workers: Vec<RemoteWorkerSummary>,
    remote_status: String,
    busy_entry_id: Option<String>,
}

#[derive(Debug, Clone)]
struct RemoteWorkerSummary {
    node_id: String,
    dispatch_endpoint_id: Option<String>,
    hostname: String,
    available: bool,
    vm_ready: bool,
    can_paint: bool,
    adapter_detail: String,
}

impl RemoteWorkerSummary {
    fn has_control_endpoint(&self) -> bool {
        !self
            .adapter_detail
            .to_ascii_lowercase()
            .contains("has not advertised a workspace worker vm control endpoint")
    }

    fn can_open_workspace(&self) -> bool {
        self.available && self.vm_ready && self.can_paint && self.has_control_endpoint()
    }

    fn blocking_detail(&self) -> &str {
        if self.adapter_detail.trim().is_empty() {
            "Worker 未报告可用的 Workspace 控制端点"
        } else {
            self.adapter_detail.as_str()
        }
    }
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
            remote_files: Vec::new(),
            workers: Vec::new(),
            remote_status: "正在检查远程打开入口…".into(),
            busy_entry_id: None,
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
                let files = search_sync_entries(
                    &state,
                    SearchSyncParams {
                        query: None,
                        status: None,
                        kind: Some("file".to_string()),
                        deleted: Some(false),
                        limit: Some(24),
                    },
                )
                .await;
                let workers = workspace_list_workers(&state).await.map(|workers| {
                    workers
                        .into_iter()
                        .map(|worker| {
                            let can_paint = worker.images.iter().any(|image| {
                                image.installed_apps.iter().any(|app| {
                                    app.trim().eq_ignore_ascii_case("paint")
                                        || app.trim().eq_ignore_ascii_case("default")
                                })
                            });
                            RemoteWorkerSummary {
                                node_id: worker.node_id,
                                dispatch_endpoint_id: worker.dispatch_endpoint_id,
                                hostname: worker.hostname,
                                available: worker.available,
                                vm_ready: worker.vm_ready,
                                can_paint,
                                adapter_detail: worker.adapter.detail,
                            }
                        })
                        .collect::<Vec<_>>()
                });
                (status, queue, files, workers)
            },
            |view, output, ctx| {
                let (status, queue, files, workers) = output;
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
                            truncate_middle(item.source_path.as_deref().unwrap_or("—"), 56,),
                            item.status
                        )
                    })
                    .collect();
                match files {
                    Ok(files) => {
                        view.remote_files = files
                            .into_iter()
                            .filter(|entry| {
                                entry
                                    .path
                                    .rsplit('.')
                                    .next()
                                    .map(|ext| {
                                        matches!(
                                            ext.to_ascii_lowercase().as_str(),
                                            "png" | "jpg" | "jpeg" | "bmp"
                                        )
                                    })
                                    .unwrap_or(false)
                            })
                            .collect();
                    }
                    Err(error) => {
                        view.remote_files.clear();
                        view.remote_status = format!("同步文件查询失败: {error}");
                    }
                }
                match workers {
                    Ok(workers) => {
                        view.workers = workers;
                        if view.remote_files.is_empty() {
                            view.remote_status = "没有可用的图片同步条目".into();
                        } else {
                            view.remote_status = format!(
                                "可远程打开的图片: {} 个；发现 worker: {} 个",
                                view.remote_files.len(),
                                view.workers.len()
                            );
                        }
                    }
                    Err(error) => {
                        view.workers.clear();
                        view.remote_status = format!("Worker 查询失败: {error}");
                    }
                }
                ctx.notify();
            },
        );
    }

    fn open_with_paint(&mut self, entry_id: String, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self
            .remote_files
            .iter()
            .find(|entry| entry.id == entry_id)
            .cloned()
        else {
            self.remote_status = "条目已不在当前同步列表中，请刷新".into();
            ctx.notify();
            return;
        };
        let author = entry
            .version_author
            .as_deref()
            .map(str::trim)
            .filter(|author| !author.is_empty() && *author != "local")
            .map(str::to_string);
        let Some(worker) = self.worker_for_entry_author(author.as_deref()) else {
            self.remote_status = if let Some(author) = author {
                format!("来源电脑 {author} 当前没有可用的 Workspace worker")
            } else {
                format!(
                    "{} 暂不能确定来源 worker，请刷新同步和 worker 状态",
                    entry.path
                )
            };
            ctx.notify();
            return;
        };
        if !worker.can_open_workspace() {
            self.remote_status = format!(
                "{} 暂不能用 Paint 打开：{}",
                worker.hostname,
                worker.blocking_detail()
            );
            ctx.notify();
            return;
        }
        let worker_node = worker.node_id.clone();
        let worker_hostname = worker.hostname.clone();
        self.busy_entry_id = Some(entry.id.clone());
        self.remote_status = format!("正在让 {} 用 Paint 打开 {}…", worker_hostname, entry.path);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let request = WorkspaceOpenRequest {
                    entry_id: entry.id,
                    cluster_id: None,
                    requested_app: Some("paint".to_string()),
                    worker_node_id: Some(worker_node.clone()),
                    policy_id: None,
                    mode: WorkspaceMode::Edit,
                };
                let session = workspace_open_file(&state, request).await?;
                workspace_start_session_worker(&state, session.session_id.clone()).await?;
                Ok::<_, String>((session.session_id, worker_node))
            },
            |view, result, ctx| {
                view.busy_entry_id = None;
                view.remote_status = match result {
                    Ok((session_id, worker_node)) => {
                        format!("已提交到来源 worker {worker_node}，会话 {session_id} 正在启动")
                    }
                    Err(error) => format!("远程打开失败: {error}"),
                };
                view.refresh(ctx);
                ctx.notify();
            },
        );
    }

    fn worker_for_author(&self, author: &str) -> Option<&RemoteWorkerSummary> {
        self.workers.iter().find(|worker| {
            worker.node_id == author || worker.dispatch_endpoint_id.as_deref() == Some(author)
        })
    }

    fn worker_for_entry_author(&self, author: Option<&str>) -> Option<&RemoteWorkerSummary> {
        if let Some(worker) = author.and_then(|author| self.worker_for_author(author)) {
            return Some(worker);
        }
        let mut candidates = self
            .workers
            .iter()
            .filter(|worker| worker.can_open_workspace());
        let first = candidates.next()?;
        if candidates.next().is_some() {
            None
        } else {
            Some(first)
        }
    }

    fn action_button(&self, label: &str, action: SyncAction, disabled: bool) -> Box<dyn Element> {
        let color = if disabled {
            theme::muted()
        } else {
            theme::accent()
        };
        let label_el = ui_text::body(label.to_string(), self.font)
            .with_color(color)
            .finish();
        let handler = EventHandler::new(label_el)
            .on_left_mouse_down(move |ctx, _, _| {
                if !disabled {
                    ctx.dispatch_typed_action(action.clone());
                }
                DispatchEventResult::StopPropagation
            })
            .finish();
        Container::new(handler)
            .with_uniform_padding(8.0)
            .with_background(theme::accent_bg(if disabled { 10 } else { 28 }))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
    }

    fn render_remote_file(&self, entry: &SyncEntryDto) -> Box<dyn Element> {
        let author = entry
            .version_author
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("未知来源");
        let worker = self.worker_for_author(author);
        let fallback_worker = (author == "local" || author == "未知来源")
            .then(|| self.worker_for_entry_author(None))
            .flatten();
        let detail = match worker {
            Some(worker) if worker.can_open_workspace() => {
                format!("来源: {} · Paint 可用", worker.hostname)
            }
            Some(worker) => format!("来源: {} · {}", worker.hostname, worker.blocking_detail()),
            None if fallback_worker.is_some() => {
                "来源待确认 · 将由后端按 SyncIndex/control-plane 校验".to_string()
            }
            None if author == "local" || author == "未知来源" => {
                "需要同步/扫描以识别来源电脑".to_string()
            }
            None => format!("来源 worker 未在线: {author}"),
        };
        let selectable_worker = worker.or(fallback_worker);
        let disabled = self.busy_entry_id.is_some()
            || selectable_worker
                .map(|worker| !worker.can_open_workspace())
                .unwrap_or(true);
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        let mut text = Flex::column();
        text.add_child(ui_text::body(entry.path.clone(), self.font).finish());
        text.add_child(
            ui_text::body(detail, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        row.add_child(
            Container::new(text.finish())
                .with_uniform_padding(6.0)
                .finish(),
        );
        row.add_child(self.action_button(
            "Paint 打开",
            SyncAction::OpenWithPaint(entry.id.clone()),
            disabled,
        ));
        Container::new(row.finish())
            .with_uniform_padding(6.0)
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish()
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
        col.add_child(section_title("同步", self.font));
        col.add_child(self.action_button("刷新", SyncAction::Refresh, self.loading));
        if self.loading {
            col.add_child(status_line("加载中…", self.font, StatusTone::Placeholder));
        } else if self.status_error {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                StatusTone::Danger,
            ));
        } else {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                StatusTone::Neutral,
            ));
        }

        col.add_child(section_title("远程打开", self.font));
        col.add_child(section_hint(
            "按 version_author 路由到来源电脑的 Workspace worker。",
            self.font,
        ));
        col.add_child(status_line(
            self.remote_status.clone(),
            self.font,
            StatusTone::Neutral,
        ));
        for entry in &self.remote_files {
            col.add_child(self.render_remote_file(entry));
        }

        col.add_child(section_title("同步队列", self.font));
        col.add_child(section_hint(
            "最近 20 条队列项。失败项请在日志或设置中排查。",
            self.font,
        ));
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

impl TypedActionView for SyncView {
    type Action = SyncAction;

    fn handle_action(&mut self, action: &SyncAction, ctx: &mut ViewContext<Self>) {
        match action {
            SyncAction::Refresh => self.refresh(ctx),
            SyncAction::OpenWithPaint(entry_id) => self.open_with_paint(entry_id.clone(), ctx),
        }
    }
}
