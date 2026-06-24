use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    ConstrainedBox, Container, DispatchEventResult, EventHandler, Flex, Image, MainAxisSize,
    ParentElement, Text,
};
use warpui::fonts::FamilyId;
use warpui::{
    assets::asset_cache::AssetCache, AppContext, Element, Entity, SingletonEntity as _,
    TypedActionView, View, ViewContext,
};
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::image_cache::{CacheOption, CustomImageFormat, CustomImageHeader, ImageType};

use crate::ui_text;

const FRAME_ASSET_ID: &str = "wormhole-computer-use-frame";
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
pub enum ComputerUseAction {
    PointerClick { x: f32, y: f32 },
}

pub struct ComputerUseView {
    frame: Arc<Mutex<SharedFrame>>,
    last_generation: u64,
    status: Arc<Mutex<String>>,
    font: FamilyId,
}

impl ComputerUseView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let frame = Arc::new(Mutex::new(SharedFrame::default()));
        let status = Arc::new(Mutex::new("正在采集屏幕…".to_string()));
        let font = crate::ui::fonts::load_ui_font(ctx);

        let view = Self {
            frame: frame.clone(),
            last_generation: 0,
            status: status.clone(),
            font,
        };
        ComputerUseView::spawn_capture(frame, status);
        view.start_poll(ctx);
        ctx.focus_self();
        view
    }

    fn spawn_capture(frame: Arc<Mutex<SharedFrame>>, status: Arc<Mutex<String>>) {
        std::thread::spawn(move || {
            let caps = computer_use::capabilities();
            if !caps.available || !caps.screenshot {
                *status.lock().expect("status") = caps.message;
                return;
            }
            let mut generation = 0u64;
            loop {
                match computer_use::capture_primary_png() {
                    Ok(png) => match image::ImageReader::new(std::io::Cursor::new(png))
                        .with_guessed_format()
                    {
                        Ok(reader) => match reader.decode() {
                            Ok(img) => {
                                let rgba = img.to_rgba8();
                                let (width, height) = rgba.dimensions();
                                let rgb = rgba
                                    .chunks_exact(4)
                                    .flat_map(|px| [px[0], px[1], px[2]])
                                    .collect::<Vec<u8>>();
                                generation += 1;
                                if let Ok(mut guard) = frame.lock() {
                                    guard.width = width;
                                    guard.height = height;
                                    guard.bytes = rgb;
                                    guard.generation = generation;
                                }
                                if generation == 1 {
                                    *status.lock().expect("status") =
                                        "看屏点选：左键点击注入指针（坐标 0–1）".to_string();
                                }
                            }
                            Err(err) => {
                                *status.lock().expect("status") = format!("解码截图失败: {err}");
                            }
                        },
                        Err(err) => {
                            *status.lock().expect("status") = format!("PNG 格式错误: {err}");
                        }
                    },
                    Err(err) => {
                        *status.lock().expect("status") = err.to_string();
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    fn normalize(position: Vector2F) -> (f64, f64) {
        (
            (position.x() / VIEW_W).clamp(0., 1.) as f64,
            (position.y() / VIEW_H).clamp(0., 1.) as f64,
        )
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(32));
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
                    let gen = view
                        .frame
                        .lock()
                        .map(|g| g.generation)
                        .unwrap_or(view.last_generation);
                    if gen != view.last_generation {
                        view.last_generation = gen;
                        view.sync_frame_texture(ctx);
                        ctx.notify();
                    }
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
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

    fn click_at(&self, x: f32, y: f32) {
        let (nx, ny) = Self::normalize(Vector2F::new(x, y));
        let status = self.status.clone();
        std::thread::spawn(move || {
            match computer_use::pointer_click(nx, ny, computer_use::PointerButton::Left) {
                Ok(()) => {
                    *status.lock().expect("status") = format!("已点击 ({nx:.3}, {ny:.3})");
                }
                Err(err) => {
                    *status.lock().expect("status") = format!("点击失败: {err}");
                }
            }
        });
    }
}

impl Entity for ComputerUseView {
    type Event = ();
}

impl View for ComputerUseView {
    fn ui_name() -> &'static str {
        "ComputerUseView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());

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
            let status = self.status.clone();
            move |_, _, position| {
                let (nx, ny) = ComputerUseView::normalize(position);
                let status = status.clone();
                std::thread::spawn(move || {
                    match computer_use::pointer_click(nx, ny, computer_use::PointerButton::Left) {
                        Ok(()) => {
                            *status.lock().expect("status") = format!("已点击 ({nx:.3}, {ny:.3})");
                        }
                        Err(err) => {
                            *status.lock().expect("status") = format!("点击失败: {err}");
                        }
                    }
                });
                DispatchEventResult::StopPropagation
            }
        })
        .finish();

        let header = Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(
                    ui_text::body(status, self.font)
                        .with_color(ColorU::new(180, 180, 180, 255))
                        .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        Flex::column()
            .with_child(header)
            .with_child(surface)
            .finish()
    }
}

impl TypedActionView for ComputerUseView {
    type Action = ComputerUseAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            ComputerUseAction::PointerClick { x, y } => {
                self.click_at(*x, *y);
                ctx.notify();
            }
        }
    }
}
