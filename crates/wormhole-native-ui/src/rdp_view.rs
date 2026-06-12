use std::sync::{Arc, Mutex};

use display_core::protocol::CodecType;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    Align, ConstrainedBox, Container, EventHandler, Flex, Image, MainAxisSize, ParentElement, Rect,
    Stack, Text,
};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    assets::asset_cache::AssetCache, AppContext, DispatchEventResult, Element, Entity,
    SingletonEntity as _, TypedActionView, View, ViewContext,
};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::elements::image::CacheOption;
use warpui_core::image_cache::{CustomImageFormat, CustomImageHeader, ImageType};
use wormhole_desktop_rdp::{decode_frame, RdpRuntime};

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
}

struct RdpInputBridge {
    session_id: Arc<Mutex<Option<String>>>,
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
}

impl RdpInputBridge {
    fn send(&self, event_type: u8, x: f32, y: f32, extra: f32) {
        let session_id = self
            .session_id
            .lock()
            .ok()
            .and_then(|g| g.clone());
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
    frame: Arc<Mutex<SharedFrame>>,
    last_generation: u64,
    status: Arc<Mutex<String>>,
    font: FamilyId,
    fullscreen: bool,
    input: Arc<RdpInputBridge>,
}

impl RdpViewerView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        peer: String,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
    ) -> Self {
        let frame = Arc::new(Mutex::new(SharedFrame::default()));
        let status = Arc::new(Mutex::new("正在连接远程桌面…".to_string()));
        let session_id = Arc::new(Mutex::new(None));
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
            .unwrap_or(FamilyId(0));

        let input = Arc::new(RdpInputBridge {
            session_id: session_id.clone(),
            runtime: runtime.clone(),
        });

        Self::spawn_session(
            runtime,
            peer.clone(),
            frame.clone(),
            status.clone(),
            session_id,
            password,
            totp_code,
            fps,
        );

        let view = Self {
            peer,
            frame,
            last_generation: 0,
            status,
            font,
            fullscreen: false,
            input,
        };
        view.start_frame_poll(ctx);
        ctx.focus_self();
        view
    }

    fn start_frame_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(16));
                if tick_tx.send_blocking(()).is_err() {
                    break;
                }
            }
        });
        Self::frame_poll_once(ctx, tick_rx);
    }

    fn frame_poll_once(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        ctx.spawn(
            async move { tick_rx.recv().await },
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
        frame: Arc<Mutex<SharedFrame>>,
        status: Arc<Mutex<String>>,
        session_id_slot: Arc<Mutex<Option<String>>>,
        password: Option<String>,
        totp_code: Option<String>,
        fps: i32,
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
                            if let Some((width, height, rgb)) = decode_frame(
                                session_id.clone(),
                                codec,
                                viewer_frame.data,
                            )
                            .await
                            {
                                generation += 1;
                                if let Ok(mut guard) = frame.lock() {
                                    guard.width = width;
                                    guard.height = height;
                                    guard.bytes = rgb;
                                    guard.generation = generation;
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
        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());
        let input = self.input.clone();

        let image = Image::new(
            AssetSource::Raw {
                id: FRAME_ASSET_ID.to_string(),
            },
            CacheOption::BySize,
        )
        .finish();

        let surface = EventHandler::new(
            ConstrainedBox::new(image)
                .with_width(VIEW_W)
                .with_height(VIEW_H)
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
        .on_right_mouse_up({
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
        .finish();

        let header = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    Text::new(status, self.font)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        Stack::new()
            .with_child(Rect::new().with_background_color(ColorU::black()).finish())
            .with_child(
                Flex::column()
                    .with_child(header)
                    .with_child(Align::new(surface).finish())
                    .finish(),
            )
            .finish()
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
        }
        self.sync_frame_texture(ctx);
        ctx.notify();
    }
}
