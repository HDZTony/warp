use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use warpui::elements::{Container, Flex, MainAxisSize, ParentElement, Text};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
};

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::rdp_extras_ui::link_label;
use crate::ui_text;

pub struct WorkspaceSessionHudView {
    session_id: String,
    title: String,
    app_name: String,
    status: String,
    status_detail: String,
    coordinator: Arc<Mutex<CoordinatorState>>,
    body: Arc<Mutex<String>>,
    generation: Arc<Mutex<u64>>,
    last_generation: u64,
    font: FamilyId,
    mono: FamilyId,
}

impl WorkspaceSessionHudView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        coordinator: Arc<Mutex<CoordinatorState>>,
        session_id: String,
        title: String,
        app_name: Option<String>,
        status: Option<String>,
        status_detail: Option<String>,
    ) -> Self {
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
            .unwrap_or(FamilyId(0));
        let mono = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .ok()
            })
            .unwrap_or(font);

        let app_name = app_name.unwrap_or_else(|| "Workspace".into());
        let status = status.unwrap_or_else(|| "unknown".into());
        let status_detail = status_detail.unwrap_or_default();
        let body = Arc::new(Mutex::new(String::new()));
        let generation = Arc::new(Mutex::new(1u64));

        let view = Self {
            session_id: session_id.clone(),
            title,
            app_name: app_name.clone(),
            status: status.clone(),
            status_detail: status_detail.clone(),
            coordinator,
            body,
            generation,
            last_generation: 0,
            font,
            mono,
        };
        view.refresh_body();
        view.start_poll(ctx);
        view
    }

    fn refresh_body(&self) {
        let text = format!(
            "Workspace 会话 HUD\n\
             ─────────────────\n\
             {}\n\
             应用：{}\n\
             状态：{}\n\
             {}\n\n\
             远程程序画面在 Workspace 原生 RDP 窗；\n\
             此窗用于状态与聚焦。\n\n\
             点击「聚焦 Workspace 窗」或按 F 聚焦远程画面。",
            self.title,
            self.app_name,
            self.status,
            if self.status_detail.is_empty() {
                "等待 VM guest 返回 RDP 端点…".to_string()
            } else {
                self.status_detail.clone()
            },
        );
        if let Ok(mut guard) = self.body.lock() {
            *guard = text;
        }
        if let Ok(mut gen) = self.generation.lock() {
            *gen = gen.saturating_add(1);
        }
    }

    fn focus_workspace(&self) {
        let window_key = crate::wormhole_native_ipc::workspace_window_key(&self.session_id);
        if let Ok(mut guard) = self.coordinator.lock() {
            guard.enqueue(UiCommand::FocusWorkspaceRdp { window_key });
        }
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
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
                    view.refresh_body();
                    ctx.notify();
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
    }
}

impl Entity for WorkspaceSessionHudView {
    type Event = ();
}

impl View for WorkspaceSessionHudView {
    fn ui_name() -> &'static str {
        "WorkspaceSessionHudView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let _ = self.generation.lock().map(|g| *g).unwrap_or(0);
        let body = self.body.lock().map(|s| s.clone()).unwrap_or_default();

        let focus = link_label("聚焦 Workspace 窗", self.font, false, {
            let this = self.session_id.clone();
            let coord = self.coordinator.clone();
            move || {
                let window_key = crate::wormhole_native_ipc::workspace_window_key(&this);
                if let Ok(mut guard) = coord.lock() {
                    guard.enqueue(UiCommand::FocusWorkspaceRdp { window_key });
                }
            }
        });

        let column = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::mono(body, self.mono)
                    .with_color(ColorU::new(220, 225, 235, 255))
                    .finish(),
            )
            .with_child(focus)
            .finish();

        Container::new(column)
            .with_uniform_padding(16.)
            .with_background_color(ColorU::new(18, 22, 30, 255))
            .finish()
    }
}

impl TypedActionView for WorkspaceSessionHudView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
