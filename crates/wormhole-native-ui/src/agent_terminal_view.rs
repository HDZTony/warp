use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use portable_pty::{native_pty_system, CommandBuilder, PtySize, PtySystem};
use vt100::Parser;
use warpui::elements::{
    ConstrainedBox, Container, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement, Text,
};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
};
use warpui_core::keymap::Keystroke;

use crate::ui_text;

const TERM_COLS: u16 = 120;
const TERM_ROWS: u16 = 36;
const VIEW_W: f32 = 1100.;
const VIEW_H: f32 = 680.;

pub struct CodexTerminalView {
    screen: Arc<Mutex<Parser>>,
    writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    status: Arc<Mutex<String>>,
    generation: Arc<Mutex<u64>>,
    last_generation: u64,
    font: FamilyId,
}

impl CodexTerminalView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        codex_binary: PathBuf,
        codex_home: PathBuf,
        api_key: String,
        profile: String,
        cwd: Option<PathBuf>,
    ) -> Self {
        let screen = Arc::new(Mutex::new(Parser::new(TERM_ROWS, TERM_COLS, 0)));
        let writer = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new("正在启动 Codex…".to_string()));
        let generation = Arc::new(Mutex::new(0u64));
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .or_else(|_| cache.load_system_font("Menlo"))
                    .ok()
            })
            .unwrap_or(FamilyId(0));

        Self::spawn_pty(
            codex_binary,
            codex_home,
            api_key,
            profile,
            cwd,
            screen.clone(),
            writer.clone(),
            status.clone(),
            generation.clone(),
        );

        let view = Self {
            screen,
            writer,
            status,
            generation,
            last_generation: 0,
            font,
        };
        view.start_poll(ctx);
        ctx.focus_self();
        view
    }

    fn spawn_pty(
        codex_binary: PathBuf,
        codex_home: PathBuf,
        api_key: String,
        profile: String,
        cwd: Option<PathBuf>,
        screen: Arc<Mutex<Parser>>,
        writer_slot: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
        status: Arc<Mutex<String>>,
        generation: Arc<Mutex<u64>>,
    ) {
        std::thread::spawn(move || {
            let pty_system = native_pty_system();
            let pair = match pty_system.openpty(PtySize {
                rows: TERM_ROWS,
                cols: TERM_COLS,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                Ok(pair) => pair,
                Err(err) => {
                    *status.lock().expect("status") = format!("无法创建 PTY: {err}");
                    return;
                }
            };

            let mut cmd = CommandBuilder::new(&codex_binary);
            cmd.arg("--profile");
            cmd.arg(&profile);
            cmd.env("CODEX_HOME", &codex_home);
            cmd.env("WORMHOLE_AGENT_API_KEY", &api_key);
            cmd.env("WORMHOLE_DEEPSEEK_API_KEY", &api_key);
            if let Some(dir) = cwd.as_ref() {
                cmd.cwd(dir);
            }

            let mut child = match pair.slave.spawn_command(cmd) {
                Ok(child) => child,
                Err(err) => {
                    *status.lock().expect("status") = format!("无法启动 Codex: {err}");
                    return;
                }
            };
            drop(pair.slave);

            let mut reader = match pair.master.try_clone_reader() {
                Ok(reader) => reader,
                Err(err) => {
                    *status.lock().expect("status") = format!("PTY 读取失败: {err}");
                    return;
                }
            };
            let writer = match pair.master.take_writer() {
                Ok(writer) => writer,
                Err(err) => {
                    *status.lock().expect("status") = format!("PTY 写入失败: {err}");
                    return;
                }
            };
            *writer_slot.lock().expect("writer") = Some(writer);
            *status.lock().expect("status") = "Codex 已启动".to_string();

            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut parser) = screen.lock() {
                            parser.process(&buf[..n]);
                        }
                        if let Ok(mut gen) = generation.lock() {
                            *gen += 1;
                        }
                    }
                    Err(err) => {
                        *status.lock().expect("status") = format!("PTY 读取错误: {err}");
                        break;
                    }
                }
            }
            let _ = child.wait();
            *status.lock().expect("status") = "Codex 已退出".to_string();
            if let Ok(mut gen) = generation.lock() {
                *gen += 1;
            }
        });
    }

    fn keystroke_to_bytes(keystroke: &Keystroke) -> Option<Vec<u8>> {
        let key = keystroke.key.as_str();
        let bytes = match key {
            "enter" | "return" => vec![b'\r'],
            "backspace" => vec![0x7f],
            "tab" => vec![b'\t'],
            "escape" => vec![0x1b],
            "up" => vec![0x1b, b'[', b'A'],
            "down" => vec![0x1b, b'[', b'B'],
            "right" => vec![0x1b, b'[', b'C'],
            "left" => vec![0x1b, b'[', b'D'],
            "home" => vec![0x1b, b'[', b'H'],
            "end" => vec![0x1b, b'[', b'F'],
            "delete" => vec![0x1b, b'[', b'3', b'~'],
            _ if key.len() == 1 => {
                let ch = key.chars().next()?;
                let mut out = ch.to_string().into_bytes();
                if keystroke.ctrl {
                    if let Some(c) = ch.to_ascii_lowercase().to_string().chars().next() {
                        if c.is_ascii_alphabetic() {
                            out = vec![(c as u8) & 0x1f];
                        }
                    }
                }
                out
            }
            _ => return None,
        };
        Some(bytes)
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
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    let gen = view
                        .generation
                        .lock()
                        .map(|g| *g)
                        .unwrap_or(view.last_generation);
                    if gen != view.last_generation {
                        view.last_generation = gen;
                        ctx.notify();
                    }
                    Self::poll_once(ctx, tick_rx);
                }
            },
        );
    }

    fn terminal_text(&self) -> String {
        let parser = match self.screen.lock() {
            Ok(p) => p,
            Err(_) => return String::new(),
        };
        parser.screen().contents()
    }
}

impl Entity for CodexTerminalView {
    type Event = ();
}

impl View for CodexTerminalView {
    fn ui_name() -> &'static str {
        "CodexTerminalView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());
        let text = self.terminal_text();
        let writer = self.writer.clone();

        let body = EventHandler::new(
            ConstrainedBox::new(
                ui_text::mono(text, self.font)
                    .with_color(ColorU::new(230, 230, 230, 255))
                    .finish(),
            )
            .with_width(VIEW_W)
            .with_height(VIEW_H)
            .finish(),
        )
        .with_always_handle()
        .on_keydown(move |_, _, keystroke| {
            if let Some(bytes) = CodexTerminalView::keystroke_to_bytes(keystroke) {
                if let Ok(mut guard) = writer.lock() {
                    if let Some(writer) = guard.as_mut() {
                        let _ = writer.write_all(&bytes);
                        let _ = writer.flush();
                    }
                }
            }
            DispatchEventResult::StopPropagation
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
            .with_child(body)
            .finish()
    }
}

impl TypedActionView for CodexTerminalView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
