use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warpui::elements::Rect;
use warpui::platform::{TerminationMode, WindowBounds};
use warpui::{
    AddWindowOptions, AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View,
    ViewContext, WindowId,
};
use warpui_core::assets::asset_cache::AssetCache;
use wormhole_desktop_rdp::RdpRuntime;

use crate::rdp_view::RdpViewerView;

#[derive(Debug, Clone)]
pub enum UiCommand {
    OpenRdp {
        peer: String,
        title: String,
        reconnect: bool,
        window_key: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    },
    FocusRdp {
        window_key: String,
        reconnect: bool,
    },
    Shutdown,
}

pub struct CoordinatorState {
    data_dir: PathBuf,
    pending: Vec<UiCommand>,
    windows: HashMap<String, WindowId>,
    rdp_runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
}

impl CoordinatorState {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir: data_dir.clone(),
            pending: Vec::new(),
            windows: HashMap::new(),
            rdp_runtime: Arc::new(tokio::sync::Mutex::new(RdpRuntime::new(data_dir))),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn enqueue(&mut self, command: UiCommand) {
        self.pending.push(command);
    }

    fn drain_commands(&mut self) -> Vec<UiCommand> {
        std::mem::take(&mut self.pending)
    }

    pub fn rdp_runtime(&self) -> Arc<tokio::sync::Mutex<RdpRuntime>> {
        self.rdp_runtime.clone()
    }
}

pub struct CoordinatorView {
    state: Arc<Mutex<CoordinatorState>>,
}

impl CoordinatorView {
    pub fn new(ctx: &mut ViewContext<Self>, state: Arc<Mutex<CoordinatorState>>) -> Self {
        AssetCache::handle(ctx).update(ctx, |_, _| {});
        ctx.focus_self();
        ctx.subscribe_to_window_events(|view_ctx, event| {
            if let warpui::event::WindowEvent::CloseRequested = event {
                view_ctx.app_mut().shutdown(TerminationMode::Graceful);
            }
        });

        let view = Self { state };
        view.start_poll(ctx);
        view.process_pending(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(16));
                if tick_tx.send_blocking(()).is_err() {
                    break;
                }
            }
        });
        Self::poll_once(ctx, tick_rx);
    }

    fn poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        ctx.spawn(
            async move { tick_rx.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.process_pending(ctx);
                    ctx.notify();
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn process_pending(&self, ctx: &mut ViewContext<Self>) {
        let commands = {
            let mut guard = self.state.lock().expect("coordinator lock");
            guard.drain_commands()
        };
        for command in commands {
            match command {
                UiCommand::OpenRdp {
                    peer,
                    title,
                    reconnect: _,
                    window_key,
                    password,
                    totp_code,
                    fps,
                } => self.open_rdp_window(
                    ctx,
                    &window_key,
                    &peer,
                    &title,
                    password,
                    totp_code,
                    fps,
                ),
                UiCommand::FocusRdp {
                    window_key,
                    reconnect: _,
                } => self.focus_window(ctx, &window_key),
                UiCommand::Shutdown => {
                    ctx.app_mut().shutdown(TerminationMode::Graceful);
                }
            }
        }
    }

    fn open_rdp_window(
        &self,
        ctx: &mut ViewContext<Self>,
        window_key: &str,
        peer: &str,
        title: &str,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) {
        let already = {
            let guard = self.state.lock().expect("coordinator lock");
            guard.windows.get(window_key).copied()
        };
        if let Some(window_id) = already {
            ctx.windows().show_window_and_focus_app(window_id);
            return;
        }

        let runtime = {
            let guard = self.state.lock().expect("coordinator lock");
            guard.rdp_runtime()
        };
        let peer = peer.to_string();
        let window_key = window_key.to_string();
        let state = self.state.clone();

        let options = AddWindowOptions {
            title: Some(title.to_string()),
            window_bounds: WindowBounds::ExactSize(vec2f(1280., 820.)),
            ..Default::default()
        };

        let (window_id, _) = ctx.app_mut().add_window(options, move |view_ctx| {
            RdpViewerView::new(view_ctx, runtime, peer, password, totp_code, fps)
        });

        {
            let mut guard = state.lock().expect("coordinator lock");
            guard.windows.insert(window_key, window_id);
        }
        ctx.windows().show_window_and_focus_app(window_id);
    }

    fn focus_window(&self, ctx: &mut ViewContext<Self>, window_key: &str) {
        let window_id = {
            let guard = self.state.lock().expect("coordinator lock");
            guard.windows.get(window_key).copied()
        };
        if let Some(window_id) = window_id {
            ctx.windows().show_window_and_focus_app(window_id);
        }
    }
}

impl Entity for CoordinatorView {
    type Event = ();
}

impl View for CoordinatorView {
    fn ui_name() -> &'static str {
        "CoordinatorView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        Rect::new()
            .with_background_color(ColorU::new(0, 0, 0, 0))
            .finish()
    }
}

impl TypedActionView for CoordinatorView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
