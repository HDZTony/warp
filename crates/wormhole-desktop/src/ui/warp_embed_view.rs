use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use pathfinder_color::ColorU;
use warpui::elements::{
    Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;

const HOST_X: i32 = 24;
const HOST_Y: i32 = 96;
const HOST_W: i32 = 1200;
const HOST_H: i32 = 640;

#[derive(Debug, Clone)]
pub enum WarpEmbedAction {
    Spawn,
    SetVisible(bool),
}

struct EmbedState {
    child: Option<Child>,
    child_hwnd: Option<isize>,
    host_hwnd: Option<isize>,
    status: String,
    spawn_attempted: bool,
}

pub struct WarpEmbedView {
    core: CoreHandle,
    font: FamilyId,
    state: Arc<Mutex<EmbedState>>,
    visible: bool,
}

impl WarpEmbedView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            font,
            state: Arc::new(Mutex::new(EmbedState {
                child: None,
                child_hwnd: None,
                host_hwnd: None,
                status: "正在准备 Warp…".to_string(),
                spawn_attempted: false,
            })),
            visible: false,
        };
        view
    }

    pub fn set_tab_visible(&mut self, visible: bool, ctx: &mut ViewContext<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            self.spawn_child(ctx);
        }
        #[cfg(windows)]
        {
            if let Some(host) = self.state.lock().expect("embed state").host_hwnd {
                wormhole_desktop_platform_windows::set_host_visible(host as _, visible);
            }
        }
        ctx.notify();
    }

    fn spawn_child(&mut self, ctx: &mut ViewContext<Self>) {
        {
            let mut state = self.state.lock().expect("embed state");
            if state.spawn_attempted {
                return;
            }
            state.spawn_attempted = true;
            state.status = "正在启动 Warp…".to_string();
        }
        ctx.notify();

        let core = self.core.clone();
        let shared = Arc::clone(&self.state);
        thread::spawn(move || {
            let launch = core.block_on(async {
                wormhole_desktop_core::warp_child_env::prepare_warp_child_launch(core.app_state())
                    .await
            });
            let launch = match launch {
                Ok(launch) => launch,
                Err(err) => {
                    shared.lock().expect("embed state").status = err;
                    return;
                }
            };

            #[cfg(windows)]
            let host_hwnd = {
                let parent = wormhole_desktop_platform_windows::find_window_by_title("Wormhole");
                parent
                    .and_then(|p| {
                        wormhole_desktop_platform_windows::create_host_panel(
                            p, HOST_X, HOST_Y, HOST_W, HOST_H,
                        )
                    })
                    .map(|hwnd| hwnd as isize)
            };
            #[cfg(not(windows))]
            let host_hwnd: Option<isize> = None;

            let mut cmd = Command::new(&launch.binary);
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            for (key, value) in &launch.env {
                cmd.env(key, value);
            }
            #[cfg(windows)]
            if let Some(host) = host_hwnd {
                cmd.env("WORMHOLE_PARENT_HWND", host.to_string());
            }

            let child = match cmd.spawn() {
                Ok(child) => child,
                Err(err) => {
                    shared.lock().expect("embed state").status =
                        format!("无法启动 Warp: {err}");
                    return;
                }
            };
            let pid = child.id();

            {
                let mut state = shared.lock().expect("embed state");
                state.child = Some(child);
                state.host_hwnd = host_hwnd;
            }

            #[cfg(windows)]
            if let Some(host) = host_hwnd {
                for _ in 0..120 {
                    if let Some(hwnd) =
                        wormhole_desktop_platform_windows::find_visible_top_level_window_for_pid(pid)
                    {
                        if wormhole_desktop_platform_windows::embed_child_into_host(hwnd, host as _)
                            .is_ok()
                        {
                            wormhole_desktop_platform_windows::resize_embedded_child(
                                hwnd, HOST_W, HOST_H,
                            );
                            let mut state = shared.lock().expect("embed state");
                            state.child_hwnd = Some(hwnd as isize);
                            state.status = "Warp 已嵌入".to_string();
                            return;
                        }
                    }
                    thread::sleep(Duration::from_millis(250));
                }
            }

            let mut state = shared.lock().expect("embed state");
            state.status = "Warp 已在独立窗口运行（嵌入失败或未就绪）".to_string();
        });
    }
}

impl Entity for WarpEmbedView {
    type Event = ();
}

impl View for WarpEmbedView {
    fn ui_name() -> &'static str {
        "WarpEmbedView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let status = self
            .state
            .lock()
            .expect("embed state")
            .status
            .clone();
        let body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::body("Warp Agent 环境", self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                ui_text::body(status, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.0,
                    Container::new(
                        warpui::elements::Text::new(
                            "完整 Warp 终端与 Codex CLI 显示在上方原生区域。",
                            self.font,
                            ui_text::BODY_SIZE,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .finish(),
                )
                .finish(),
            );
        Container::new(body.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}

impl TypedActionView for WarpEmbedView {
    type Action = WarpEmbedAction;

    fn handle_action(&mut self, action: &WarpEmbedAction, ctx: &mut ViewContext<Self>) {
        match action {
            WarpEmbedAction::Spawn => self.spawn_child(ctx),
            WarpEmbedAction::SetVisible(visible) => self.set_tab_visible(*visible, ctx),
        }
    }
}

impl Drop for WarpEmbedView {
    fn drop(&mut self) {
        let mut state = self.state.lock().expect("embed state");
        if let Some(mut child) = state.child.take() {
            let _ = child.kill();
        }
        #[cfg(windows)]
        if let Some(host) = state.host_hwnd.take() {
            wormhole_desktop_platform_windows::destroy_host(host as _);
        }
    }
}
