use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use serde::Deserialize;
use serde_json::Value;
use warpui::elements::{ConstrainedBox, Container, Flex, MainAxisSize, ParentElement, Text};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
};

use crate::ui_text;
use wormhole_desktop_rdp::RdpRuntime;

const VIEW_W: f32 = 1120.;
const VIEW_H: f32 = 760.;
const EVENTS_H: f32 = 560.;
const SIDEBAR_W: f32 = 300.;

#[derive(Debug, Deserialize)]
struct EventsPage {
    events: Vec<Value>,
    next_cursor: u64,
}

pub struct AgentEventsView {
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    target_node: String,
    task_id: String,
    events_log: Arc<Mutex<String>>,
    sidebar: Arc<Mutex<String>>,
    status: Arc<Mutex<String>>,
    cursor: Arc<Mutex<u64>>,
    generation: Arc<Mutex<u64>>,
    last_generation: u64,
    font: FamilyId,
    mono: FamilyId,
}

impl AgentEventsView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        target_node: String,
        task_id: String,
    ) -> Self {
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
            .unwrap_or(FamilyId(0));
        let mono = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .or_else(|_| cache.load_system_font("Menlo"))
                    .ok()
            })
            .unwrap_or(font);

        let events_log = Arc::new(Mutex::new(format!(
            "远程 Agent 任务事件 · {task_id}\n目标节点: {target_node}\n\n"
        )));
        let sidebar = Arc::new(Mutex::new("任务详情\n────────\n等待事件…\n".to_string()));
        let status = Arc::new(Mutex::new("轮询事件流…".to_string()));
        let cursor = Arc::new(Mutex::new(0u64));
        let generation = Arc::new(Mutex::new(1u64));

        let view = Self {
            runtime: runtime.clone(),
            target_node: target_node.clone(),
            task_id: task_id.clone(),
            events_log,
            sidebar,
            status,
            cursor,
            generation,
            last_generation: 0,
            font,
            mono,
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::poll_once(ctx, tick_rx);
    }

    fn poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.poll_events();
                    Self::bump_generation(&view.generation);
                    ctx.notify();
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn bump_generation(slot: &Arc<Mutex<u64>>) {
        if let Ok(mut guard) = slot.lock() {
            *guard = guard.saturating_add(1);
        }
    }

    fn poll_events(&self) {
        let since = *self.cursor.lock().unwrap_or_else(|e| e.into_inner());
        let runtime = self.runtime.clone();
        let target_node = self.target_node.clone();
        let task_id = self.task_id.clone();
        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(async move {
                let runtime = runtime.lock().await;
                runtime
                    .remote_agent_read_events(&target_node, &task_id, Some(since))
                    .await
                    .ok()
            })
        })
        .join()
        .ok()
        .flatten();

        let Some(page_value) = result else {
            *self.status.lock().unwrap_or_else(|e| e.into_inner()) =
                "读取事件失败（检查节点与 task_id）".to_string();
            return;
        };

        let page: EventsPage = match serde_json::from_value(page_value) {
            Ok(page) => page,
            Err(err) => {
                *self.status.lock().unwrap_or_else(|e| e.into_inner()) =
                    format!("解析事件失败: {err}");
                return;
            }
        };

        if page.events.is_empty() {
            *self.status.lock().unwrap_or_else(|e| e.into_inner()) =
                format!("已同步 · cursor={}", page.next_cursor);
            return;
        }

        let mut log = self.events_log.lock().unwrap_or_else(|e| e.into_inner());
        for event in &page.events {
            let timestamp = event.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            let level = event
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info")
                .to_uppercase();
            let message = event.get("message").and_then(|v| v.as_str()).unwrap_or("");
            log.push_str(&format!("[{timestamp}] {level} {message}\n"));
        }
        drop(log);

        *self.cursor.lock().unwrap_or_else(|e| e.into_inner()) = page.next_cursor;
        *self.sidebar.lock().unwrap_or_else(|e| e.into_inner()) = format!(
            "任务详情\n────────\ntask_id: {}\ntarget: {}\n\ncursor: {}\nevents: +{}\n",
            self.task_id,
            self.target_node,
            page.next_cursor,
            page.events.len(),
        );
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) =
            format!("已接收 {} 条事件", page.events.len());
    }
}

impl Entity for AgentEventsView {
    type Event = ();
}

impl View for AgentEventsView {
    fn ui_name() -> &'static str {
        "AgentEventsView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let gen = self
            .generation
            .lock()
            .map(|g| *g)
            .unwrap_or(self.last_generation);
        let _ = gen;

        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());
        let events = self
            .events_log
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| String::new());
        let sidebar = self
            .sidebar
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| String::new());

        let header = Container::new(
            ui_text::body(status, self.font)
                .with_color(ColorU::new(200, 200, 200, 255))
                .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        let events_body = Container::new(
            ConstrainedBox::new(
                ui_text::mono(events, self.mono)
                    .with_color(ColorU::new(220, 220, 220, 255))
                    .finish(),
            )
            .with_width(VIEW_W - SIDEBAR_W - 24.)
            .with_height(EVENTS_H)
            .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        let sidebar_body = Container::new(
            ConstrainedBox::new(
                ui_text::mono(sidebar, self.mono)
                    .with_color(ColorU::new(170, 190, 210, 255))
                    .finish(),
            )
            .with_width(SIDEBAR_W)
            .with_height(EVENTS_H)
            .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        let split = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(events_body)
            .with_child(sidebar_body)
            .finish();

        Flex::column().with_child(header).with_child(split).finish()
    }
}

impl TypedActionView for AgentEventsView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
