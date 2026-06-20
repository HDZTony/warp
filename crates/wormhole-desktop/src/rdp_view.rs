use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use display_core::protocol::CodecType;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    Align, ConstrainedBox, Container, DispatchEventResult, EventHandler, Flex, Image, MainAxisSize,
    ParentElement, Rect, Stack, Text,
};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    assets::asset_cache::AssetCache, AppContext, Element, Entity, SingletonEntity as _,
    TypedActionView, View, ViewContext,
};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::{CacheOption, CustomImageFormat, CustomImageHeader, ImageType};
use warpui_core::keymap::Keystroke;
use wormhole_desktop_rdp::{decode_frame, ExtrasStateHandle, RdpRuntime};

use crate::rdp_extras_ui::{
    self, apply_keystroke, render_auth_panel, spawn_audio_muted, spawn_audio_volume,
    spawn_open_tunnel, spawn_run_terminal, spawn_send_file, ActiveField, ExtrasPanel,
    ExtrasUiAction, ExtrasUiState, ViewerScale,
};
use crate::ui_text;
#[cfg(target_os = "macos")]
use warpui_core::platform::file_picker::FilePickerConfiguration;

const FRAME_ASSET_ID: &str = "wormhole-rdp-frame";
const DEFAULT_FPS: i32 = 60;
const VIEW_W: f32 = 1280.;
const VIEW_H: f32 = 720.;

#[derive(Default)]
struct SharedFrame {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    generation: u64,
}

#[derive(Debug, Clone)]
pub enum RdpAction {
    Pointer {
        event_type: u8,
        x: f32,
        y: f32,
        extra: f32,
    },
    ToggleFullscreen,
    ExtrasUi(ExtrasUiAction),
    ExtrasKeydown(Keystroke),
}

struct RdpInputBridge {
    session_id: Arc<Mutex<Option<String>>>,
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
}

impl RdpInputBridge {
    fn send(&self, event_type: u8, x: f32, y: f32, extra: f32) {
        let session_id = self.session_id.lock().ok().and_then(|g| g.clone());
        let Some(session_id) = session_id else { return };
        let runtime = self.runtime.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("input runtime");
            let _ = rt.block_on(async move {
                let runtime = runtime.lock().await;
                let _ = runtime
                    .send_input(&session_id, event_type, x, y, extra, 1.0, 0.0, 0.0)
                    .await;
            });
        });
    }
}

pub struct RdpViewerView {
    peer: String,
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    session_id: Arc<Mutex<Option<String>>>,
    frame: Arc<Mutex<SharedFrame>>,
    last_generation: u64,
    status: Arc<Mutex<String>>,
    font: FamilyId,
    mono_font: FamilyId,
    fullscreen: bool,
    input: Arc<RdpInputBridge>,
    watermark_text: Option<String>,
    watch_only: bool,
    show_extras_tools: bool,
    show_audio_controls: bool,
    extras: ExtrasStateHandle,
    extras_ui: Arc<Mutex<ExtrasUiState>>,
    ui_generation: Arc<Mutex<u64>>,
    connect_fps: i32,
    data_dir: PathBuf,
}

impl RdpViewerView {
    pub fn new_workspace(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        password: String,
        fps: i32,
        watermark_text: Option<String>,
    ) -> Self {
        Self::new_internal(
            ctx,
            runtime,
            peer,
            Some(password),
            None,
            fps,
            watermark_text,
            false,
            false,
            false,
        )
    }

    pub fn new(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) -> Self {
        Self::new_internal(
            ctx, runtime, peer, password, totp_code, fps, None, false, true, false,
        )
    }

    pub fn new_live(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) -> Self {
        Self::new_internal(
            ctx,
            runtime,
            peer,
            password,
            totp_code,
            fps,
            Some("P2P 视频 · 仅观看".into()),
            true,
            false,
            true,
        )
    }

    fn new_internal(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
        watermark_text: Option<String>,
        watch_only: bool,
        show_extras_tools: bool,
        show_audio_controls: bool,
    ) -> Self {
        let frame = Arc::new(Mutex::new(SharedFrame::default()));
        let status = Arc::new(Mutex::new("正在连接远程桌面…".to_string()));
        let session_id = Arc::new(Mutex::new(None));
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
            .unwrap_or(FamilyId(0));
        let mono_font = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .or_else(|_| cache.load_system_font("Menlo"))
                    .ok()
            })
            .unwrap_or(font);

        let input = Arc::new(RdpInputBridge {
            session_id: session_id.clone(),
            runtime: runtime.clone(),
        });
        let extras = {
            let runtime = runtime.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("extras runtime");
                rt.block_on(async {
                    let guard = runtime.lock().await;
                    guard.extras()
                })
            })
            .join()
            .expect("extras join")
        };

        let spawn_runtime = runtime.clone();
        let extras_ui = Arc::new(Mutex::new(ExtrasUiState::new()));
        let ui_generation = Arc::new(Mutex::new(1u64));

        let data_dir = {
            let runtime = runtime.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("data_dir runtime");
                rt.block_on(async { runtime.lock().await.data_dir().to_path_buf() })
            })
            .join()
            .expect("data_dir join")
        };
        if let Some((available, hint)) = crate::shell_bridge::virtual_cam_config(&data_dir) {
            if let Ok(mut guard) = extras_ui.lock() {
                guard.virtual_cam_available = available;
                guard.virtual_cam_hint = hint;
            }
        }

        Self::spawn_session(
            spawn_runtime,
            peer.clone(),
            data_dir.clone(),
            frame.clone(),
            status.clone(),
            session_id.clone(),
            password,
            totp_code,
            fps,
            Some(extras_ui.clone()),
        );

        let view = Self {
            peer,
            runtime,
            session_id,
            frame,
            last_generation: 0,
            status,
            font,
            mono_font,
            fullscreen: false,
            input,
            watermark_text,
            watch_only,
            show_extras_tools,
            show_audio_controls,
            extras,
            extras_ui,
            ui_generation,
            connect_fps: fps,
            data_dir,
        };
        if show_audio_controls {
            view.start_audio_poll(ctx);
        }
        view.start_frame_poll(ctx);
        ctx.focus_self();
        view
    }

    fn bump_ui(&self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut guard) = self.ui_generation.lock() {
            *guard = guard.saturating_add(1);
        }
        ctx.notify();
    }

    fn start_audio_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(1200));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::audio_poll_once(ctx, tick_rx);
    }

    fn audio_poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.poll_audio_stats();
                    view.bump_ui(ctx);
                    Self::audio_poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn poll_audio_stats(&self) {
        let session_id = self.session_id.lock().ok().and_then(|g| g.clone());
        let Some(session_id) = session_id else { return };
        let runtime = self.runtime.clone();
        let extras_ui = self.extras_ui.clone();
        let stats = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(async move {
                let runtime = runtime.lock().await;
                runtime.viewer_audio_stats(&session_id).await.ok()
            })
        })
        .join()
        .ok()
        .flatten();
        let Some(stats) = stats else { return };
        let Ok(mut guard) = extras_ui.lock() else {
            return;
        };
        guard.audio_volume = stats.volume;
        guard.audio_muted = stats.muted;
        guard.audio_has_stream = stats.has_stream;
    }

    fn clone_dispatch(&self) -> RdpExtrasDispatch {
        RdpExtrasDispatch {
            peer: self.peer.clone(),
            runtime: self.runtime.clone(),
            extras_ui: self.extras_ui.clone(),
            ui_generation: self.ui_generation.clone(),
            data_dir: self.data_dir.clone(),
            frame: self.frame.clone(),
        }
    }

    fn on_extras_keystroke(&self, keystroke: &Keystroke, ctx: &mut ViewContext<Self>) {
        let (has_panel, auth_prompt) = self
            .extras_ui
            .lock()
            .map(|g| (g.panel.is_some(), g.auth_prompt))
            .unwrap_or((false, false));
        if !has_panel && !auth_prompt {
            return;
        }
        let submit = apply_keystroke(&self.extras_ui, keystroke);
        if submit {
            if auth_prompt {
                self.reconnect_session(ctx);
            } else {
                self.clone_dispatch().apply(ExtrasUiAction::Submit);
            }
        }
        self.bump_ui(ctx);
    }

    fn disconnect_session(&self, ctx: &mut ViewContext<Self>) {
        let session_id = self.session_id.lock().ok().and_then(|g| g.clone());
        if let Some(session_id) = session_id {
            let runtime = self.runtime.clone();
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    let _ = rt.block_on(async move {
                        let runtime = runtime.lock().await;
                        runtime.stop_session(&session_id).await
                    });
                }
            });
        }
        if let Ok(mut guard) = self.session_id.lock() {
            *guard = None;
        }
        if let Ok(mut status) = self.status.lock() {
            *status = "已断开".into();
        }
        self.bump_ui(ctx);
    }

    fn send_ctrl_alt_del(&self) {
        let session_id = self.session_id.lock().ok().and_then(|g| g.clone());
        let Some(session_id) = session_id else { return };
        let runtime = self.runtime.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                let _ = rt.block_on(async move {
                    let runtime = runtime.lock().await;
                    let _ = runtime.send_ctrl_alt_del(&session_id).await;
                });
            }
        });
    }

    fn reconnect_session(&self, ctx: &mut ViewContext<Self>) {
        let (password, totp) = self
            .extras_ui
            .lock()
            .map(|g| {
                (
                    (!g.auth_password.is_empty()).then(|| g.auth_password.clone()),
                    (!g.auth_totp.is_empty()).then(|| g.auth_totp.clone()),
                )
            })
            .unwrap_or((None, None));
        if let Ok(mut guard) = self.session_id.lock() {
            *guard = None;
        }
        if let Ok(mut ui) = self.extras_ui.lock() {
            ui.auth_prompt = false;
        }
        if let Ok(mut status) = self.status.lock() {
            *status = "正在重新连接…".into();
        }
        Self::spawn_session(
            self.runtime.clone(),
            self.peer.clone(),
            self.data_dir.clone(),
            self.frame.clone(),
            self.status.clone(),
            self.session_id.clone(),
            password,
            totp,
            self.connect_fps,
            Some(self.extras_ui.clone()),
        );
        self.bump_ui(ctx);
    }

    fn open_file_picker_mac(&self, ctx: &mut ViewContext<Self>) {
        #[cfg(target_os = "macos")]
        {
            let extras_ui = self.extras_ui.clone();
            ctx.open_file_picker(
                move |result, _ctx| {
                    if let Ok(paths) = result {
                        if let Some(path) = paths.first() {
                            if let Ok(mut guard) = extras_ui.lock() {
                                guard.file_path = path.clone();
                            }
                        }
                    }
                },
                FilePickerConfiguration::new(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = ctx;
            if let Ok(mut guard) = self.extras_ui.lock() {
                guard.active_field = ActiveField::FilePath;
            }
            self.bump_ui(ctx);
        }
    }

    fn start_frame_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::frame_poll_once(ctx, tick_rx);
    }

    fn frame_poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.sync_frame_texture(ctx);
                    ctx.notify();
                    Self::frame_poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn spawn_session(
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        data_dir: PathBuf,
        frame: Arc<Mutex<SharedFrame>>,
        status: Arc<Mutex<String>>,
        session_id_slot: Arc<Mutex<Option<String>>>,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
        extras_ui: Option<Arc<Mutex<ExtrasUiState>>>,
    ) {
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    *status.lock().expect("status lock") = format!("无法启动会话运行时: {err}");
                    return;
                }
            };
            rt.block_on(async move {
                let session_id = {
                    let runtime = runtime.lock().await;
                    match runtime
                        .start_viewer_session(
                            &peer,
                            fps.max(1),
                            password.as_deref(),
                            totp_code.as_deref(),
                        )
                        .await
                    {
                        Ok(id) => id,
                        Err(err) => {
                            *status.lock().expect("status lock") = format!("连接失败: {err}");
                            if let Some(ui) = extras_ui.as_ref() {
                                if let Ok(mut guard) = ui.lock() {
                                    guard.auth_prompt = true;
                                    guard.active_field = ActiveField::AuthPassword;
                                }
                            }
                            return;
                        }
                    }
                };
                *session_id_slot.lock().expect("session lock") = Some(session_id.clone());
                *status.lock().expect("status lock") = format!("已连接 · {peer}");

                let viewer = {
                    let runtime = runtime.lock().await;
                    match runtime.viewer(&session_id).await {
                        Ok(v) => v,
                        Err(err) => {
                            *status.lock().expect("status lock") =
                                format!("无法获取 viewer: {err}");
                            return;
                        }
                    }
                };

                let mut rx = viewer.subscribe_frames();
                let mut generation = 0u64;
                loop {
                    match rx.recv().await {
                        Ok(viewer_frame) => {
                            let codec = match viewer_frame.codec.as_str() {
                                "hevc" => CodecType::Hevc,
                                "av1" => CodecType::Av1,
                                _ => CodecType::H264,
                            };
                            if let Some((width, height, rgb)) =
                                decode_frame(session_id.clone(), codec, viewer_frame.data).await
                            {
                                generation += 1;
                                if let Ok(mut guard) = frame.lock() {
                                    guard.width = width;
                                    guard.height = height;
                                    guard.bytes = rgb.clone();
                                    guard.generation = generation;
                                }
                                let push_vcam = extras_ui.as_ref().and_then(|ui| {
                                    ui.lock()
                                        .ok()
                                        .filter(|g| {
                                            g.virtual_cam_enabled && g.virtual_cam_available
                                        })
                                        .map(|_| ())
                                });
                                if push_vcam.is_some() {
                                    let data_dir = data_dir.clone();
                                    std::thread::spawn(move || {
                                        let _ = crate::shell_bridge::push_virtual_cam_rgb(
                                            &data_dir, width, height, &rgb,
                                        );
                                    });
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => break,
                    }
                }
            });
        });
    }

    fn normalize(position: Vector2F) -> (f32, f32) {
        (
            (position.x() / VIEW_W).clamp(0., 1.),
            (position.y() / VIEW_H).clamp(0., 1.),
        )
    }

    fn sync_frame_texture(&mut self, ctx: &mut ViewContext<Self>) {
        let (generation, width, height, bytes) = {
            let guard = self.frame.lock().expect("frame lock");
            (
                guard.generation,
                guard.width,
                guard.height,
                guard.bytes.clone(),
            )
        };
        if generation == 0 || generation == self.last_generation {
            return;
        }
        self.last_generation = generation;
        if let Ok(payload) =
            CustomImageHeader::prepend_custom_header(bytes, width, height, CustomImageFormat::Rgb)
        {
            AssetCache::handle(ctx).update(ctx, |cache, model_ctx| {
                cache.insert_raw_asset_bytes::<ImageType>(
                    FRAME_ASSET_ID.to_string(),
                    &payload,
                    model_ctx,
                );
            });
        }
    }
}

impl Entity for RdpViewerView {
    type Event = ();
}

impl View for RdpViewerView {
    fn ui_name() -> &'static str {
        "RdpViewerView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let _ui_gen = self.ui_generation.lock().map(|g| *g).unwrap_or(0);
        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());
        let header_text = if self.watch_only {
            status
        } else {
            self.extras
                .extras_line_for_peer(&self.peer)
                .map(|line| format!("{status} · {line}"))
                .unwrap_or(status)
        };
        let input = self.input.clone();

        let (viewer_scale, frame_w, frame_h) = {
            let scale = self
                .extras_ui
                .lock()
                .map(|g| g.viewer_scale)
                .unwrap_or(ViewerScale::Fit);
            let guard = self.frame.lock().expect("frame lock");
            (scale, guard.width.max(1), guard.height.max(1))
        };
        let (surface_w, surface_h) = match viewer_scale {
            ViewerScale::OneToOne => (frame_w as f32, frame_h as f32),
            ViewerScale::Fit | ViewerScale::Fill => (VIEW_W, VIEW_H),
        };

        let image = Image::new(
            AssetSource::Raw {
                id: FRAME_ASSET_ID.to_string(),
            },
            CacheOption::BySize,
        )
        .finish();

        let surface = if self.watch_only {
            ConstrainedBox::new(image)
                .with_width(surface_w)
                .with_height(surface_h)
                .finish()
        } else {
            EventHandler::new(
                ConstrainedBox::new(image)
                    .with_width(surface_w)
                    .with_height(surface_h)
                    .finish(),
            )
            .on_left_mouse_down({
                let input = input.clone();
                move |_, _, position| {
                    let (x, y) = RdpViewerView::normalize(position);
                    input.send(0, x, y, 0.);
                    DispatchEventResult::StopPropagation
                }
            })
            .on_left_mouse_up({
                let input = input.clone();
                move |_, _, position| {
                    let (x, y) = RdpViewerView::normalize(position);
                    input.send(2, x, y, 0.);
                    DispatchEventResult::StopPropagation
                }
            })
            .on_right_mouse_down({
                let input = input.clone();
                move |_, _, position| {
                    let (x, y) = RdpViewerView::normalize(position);
                    input.send(7, x, y, 0.);
                    DispatchEventResult::StopPropagation
                }
            })
            .on_right_mouse_down({
                let input = input.clone();
                move |_, _, position| {
                    let (x, y) = RdpViewerView::normalize(position);
                    input.send(8, x, y, 0.);
                    DispatchEventResult::StopPropagation
                }
            })
            .on_mouse_dragged({
                let input = input.clone();
                move |_, _, position| {
                    let (x, y) = RdpViewerView::normalize(position);
                    input.send(1, x, y, 0.);
                    DispatchEventResult::StopPropagation
                }
            })
            .on_scroll_wheel({
                let input = input.clone();
                move |_, _, position, _| {
                    let (x, y) = RdpViewerView::normalize(*position);
                    input.send(3, x, y, position.y());
                    DispatchEventResult::StopPropagation
                }
            })
            .finish()
        };

        let dispatch = self.clone_dispatch();
        let on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync> =
            Arc::new(move |action| dispatch.apply(action));
        let toolbar = rdp_extras_ui::render_toolbar(
            self.font,
            self.show_extras_tools,
            self.show_audio_controls,
            true,
            self.watch_only,
            &self.extras_ui,
            on_action.clone(),
        );
        let mut header_column = Flex::column()
            .with_child(
                ui_text::body(header_text, self.font)
                    .with_color(ColorU::white())
                    .finish(),
            )
            .with_child(toolbar);
        if let Some(auth) = render_auth_panel(
            self.font,
            self.mono_font,
            &self.extras_ui,
            on_action.clone(),
        ) {
            header_column = header_column.with_child(auth);
        }
        if self.show_extras_tools {
            if let Some(panel) =
                rdp_extras_ui::render_panel(self.font, self.mono_font, &self.extras_ui, on_action)
            {
                header_column = header_column.with_child(panel);
            }
        }

        let header = Container::new(header_column.finish())
            .with_uniform_padding(8.)
            .finish();

        let mut stack = Stack::new()
            .with_child(Rect::new().with_background_color(ColorU::black()).finish())
            .with_child(
                Flex::column()
                    .with_child(header)
                    .with_child(Align::new(surface).finish())
                    .finish(),
            );

        if let Some(label) = self.watermark_text.as_ref().filter(|s| !s.is_empty()) {
            let wm = label.clone();
            let watermark = Flex::column()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Container::new(
                        ui_text::body(wm, self.font)
                            .with_color(ColorU::new(255, 255, 255, 70))
                            .finish(),
                    )
                    .with_uniform_padding(48.)
                    .finish(),
                )
                .finish();
            stack = stack.with_child(watermark);
        }

        stack.finish()
    }
}

impl TypedActionView for RdpViewerView {
    type Action = RdpAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            RdpAction::Pointer {
                event_type,
                x,
                y,
                extra,
            } => self.input.send(*event_type, *x, *y, *extra),
            RdpAction::ToggleFullscreen => {
                ctx.toggle_fullscreen();
                self.fullscreen = !self.fullscreen;
            }
            RdpAction::ExtrasUi(action) => match action {
                ExtrasUiAction::ToggleFullscreen => {
                    ctx.toggle_fullscreen();
                    self.fullscreen = !self.fullscreen;
                }
                ExtrasUiAction::Disconnect => self.disconnect_session(ctx),
                ExtrasUiAction::CtrlAltDel => self.send_ctrl_alt_del(),
                ExtrasUiAction::Reconnect => self.reconnect_session(ctx),
                ExtrasUiAction::PickFile => self.open_file_picker_mac(ctx),
                other => {
                    self.clone_dispatch().apply(other.clone());
                    self.bump_ui(ctx);
                }
            },
            RdpAction::ExtrasKeydown(keystroke) => self.on_extras_keystroke(keystroke, ctx),
        }
        self.sync_frame_texture(ctx);
        ctx.notify();
    }
}

#[derive(Clone)]
struct RdpExtrasDispatch {
    peer: String,
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    extras_ui: Arc<Mutex<ExtrasUiState>>,
    ui_generation: Arc<Mutex<u64>>,
    data_dir: PathBuf,
    frame: Arc<Mutex<SharedFrame>>,
}

impl RdpExtrasDispatch {
    fn bump(&self) {
        if let Ok(mut guard) = self.ui_generation.lock() {
            *guard = guard.saturating_add(1);
        }
    }

    fn apply(&self, action: ExtrasUiAction) {
        match action {
            ExtrasUiAction::TogglePanel(panel) => {
                if let Ok(mut guard) = self.extras_ui.lock() {
                    guard.panel = if guard.panel == Some(panel) {
                        None
                    } else {
                        Some(panel)
                    };
                    guard.active_field = match panel {
                        ExtrasPanel::File => ActiveField::FilePath,
                        ExtrasPanel::Tunnel => ActiveField::TunnelLocal,
                        ExtrasPanel::Terminal => ActiveField::TerminalCmd,
                    };
                }
                self.bump();
            }
            ExtrasUiAction::FocusField(field) => {
                if let Ok(mut guard) = self.extras_ui.lock() {
                    guard.active_field = field;
                }
                self.bump();
            }
            ExtrasUiAction::PickFile => {}
            ExtrasUiAction::Submit => self.submit_panel(),
            ExtrasUiAction::ToggleMute => {
                let muted = self
                    .extras_ui
                    .lock()
                    .map(|mut g| {
                        g.audio_muted = !g.audio_muted;
                        g.audio_muted
                    })
                    .unwrap_or(false);
                spawn_audio_muted(self.runtime.clone(), muted);
                self.bump();
            }
            ExtrasUiAction::VolumeDelta(delta) => {
                let volume = self
                    .extras_ui
                    .lock()
                    .map(|mut g| {
                        let next = (g.audio_volume as i16 + delta as i16).clamp(0, 100) as u8;
                        g.audio_volume = next;
                        next
                    })
                    .unwrap_or(100);
                spawn_audio_volume(self.runtime.clone(), volume);
                self.bump();
            }
            ExtrasUiAction::CycleScale => {
                if let Ok(mut guard) = self.extras_ui.lock() {
                    guard.viewer_scale = guard.viewer_scale.cycle();
                }
                self.bump();
            }
            ExtrasUiAction::ToggleVirtualCam => {
                let (enabled, width, height) = self
                    .extras_ui
                    .lock()
                    .map(|mut g| {
                        g.virtual_cam_enabled = !g.virtual_cam_enabled;
                        let (w, h) = self
                            .frame
                            .lock()
                            .map(|f| (f.width.max(640), f.height.max(360)))
                            .unwrap_or((1280, 720));
                        (g.virtual_cam_enabled, w, h)
                    })
                    .unwrap_or((false, 1280, 720));
                let data_dir = self.data_dir.clone();
                std::thread::spawn(move || {
                    let _ = crate::shell_bridge::set_virtual_cam(&data_dir, enabled, width, height);
                });
                self.bump();
            }
            ExtrasUiAction::ToggleFullscreen
            | ExtrasUiAction::Disconnect
            | ExtrasUiAction::CtrlAltDel
            | ExtrasUiAction::Reconnect => {}
        }
    }

    fn submit_panel(&self) {
        let snapshot = self.extras_ui.lock().ok().map(|g| {
            (
                g.panel,
                g.file_path.clone(),
                g.tunnel_local.clone(),
                g.tunnel_remote.clone(),
                g.terminal_cmd.clone(),
            )
        });
        let Some((panel, file_path, tunnel_local, tunnel_remote, terminal_cmd)) = snapshot else {
            return;
        };
        let peer = self.peer.clone();
        let runtime = self.runtime.clone();
        match panel {
            Some(ExtrasPanel::File) => {
                let path = file_path.trim().to_string();
                if !path.is_empty() {
                    spawn_send_file(runtime, peer, path);
                }
            }
            Some(ExtrasPanel::Tunnel) => {
                spawn_open_tunnel(runtime, peer, tunnel_local, tunnel_remote);
            }
            Some(ExtrasPanel::Terminal) => {
                let cmd = terminal_cmd.trim().to_string();
                if !cmd.is_empty() {
                    spawn_run_terminal(runtime, peer, cmd);
                }
            }
            None => {}
        }
        self.bump();
    }
}
