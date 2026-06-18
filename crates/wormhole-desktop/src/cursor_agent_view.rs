use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use serde::{Deserialize, Serialize};
use warpui::elements::{
    ConstrainedBox, Container, DispatchEventResult, EventHandler, Flex, ParentElement, Text,
};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
};
use warpui_core::keymap::Keystroke;

use crate::ui_text;

const VIEW_W: f32 = 1100.;
const VIEW_H: f32 = 680.;
const INPUT_H: f32 = 36.;

#[derive(Debug, Clone)]
pub enum CursorAgentAction {
    Keydown(Keystroke),
}

#[derive(Debug, Serialize)]
struct CursorScriptInput<'a> {
    api_key: &'a str,
    model: &'a str,
    cwd: &'a str,
    prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_id: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct CursorScriptOutput {
    ok: bool,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct CursorSessionState {
    #[serde(default)]
    agent_id: Option<String>,
}

pub struct CursorAgentView {
    data_dir: PathBuf,
    session_key: String,
    node_binary: PathBuf,
    cursor_script: PathBuf,
    cursor_workdir: PathBuf,
    api_key: String,
    model: String,
    log: Arc<Mutex<String>>,
    input: Arc<Mutex<String>>,
    status: Arc<Mutex<String>>,
    busy: Arc<Mutex<bool>>,
    generation: Arc<Mutex<u64>>,
    last_generation: u64,
    font: FamilyId,
}

impl CursorAgentView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        data_dir: PathBuf,
        session_key: String,
        node_binary: PathBuf,
        cursor_script: PathBuf,
        cursor_workdir: PathBuf,
        api_key: String,
        model: String,
    ) -> Self {
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .or_else(|_| cache.load_system_font("Menlo"))
                    .ok()
            })
            .unwrap_or(FamilyId(0));

        let mut log_text =
            String::from("Cursor Agent — 输入任务后按 Enter 发送（多轮对话自动 resume）。\n\n");
        if let Some(agent_id) = Self::load_agent_id(&data_dir, &session_key) {
            log_text.push_str(&format!("已恢复会话 agentId={agent_id}\n\n"));
        }

        let view = Self {
            data_dir,
            session_key,
            node_binary,
            cursor_script,
            cursor_workdir,
            api_key,
            model,
            log: Arc::new(Mutex::new(log_text)),
            input: Arc::new(Mutex::new(String::new())),
            status: Arc::new(Mutex::new("就绪".to_string())),
            busy: Arc::new(Mutex::new(false)),
            generation: Arc::new(Mutex::new(0)),
            last_generation: 0,
            font,
        };
        view.start_poll(ctx);
        ctx.focus_self();
        view
    }

    fn session_path(data_dir: &Path, session_key: &str) -> PathBuf {
        data_dir
            .join("native-ui")
            .join("cursor-sessions")
            .join(format!("{session_key}.json"))
    }

    fn load_agent_id(data_dir: &Path, session_key: &str) -> Option<String> {
        let path = Self::session_path(data_dir, session_key);
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str::<CursorSessionState>(&raw)
            .ok()
            .and_then(|state| state.agent_id)
            .filter(|id| Self::is_safe_agent_id(id))
    }

    fn save_agent_id(data_dir: &Path, session_key: &str, agent_id: &str) -> Result<(), String> {
        if !Self::is_safe_agent_id(agent_id) {
            return Err("invalid agent id".into());
        }
        let path = Self::session_path(data_dir, session_key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir cursor session: {e}"))?;
        }
        let payload = CursorSessionState {
            agent_id: Some(agent_id.to_string()),
        };
        let raw = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("encode cursor session: {e}"))?;
        std::fs::write(path, raw).map_err(|e| format!("write cursor session: {e}"))
    }

    fn is_safe_agent_id(agent_id: &str) -> bool {
        !agent_id.is_empty()
            && agent_id.len() <= 128
            && agent_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn append_log(log: &Arc<Mutex<String>>, line: &str) {
        if let Ok(mut guard) = log.lock() {
            guard.push_str(line);
            if !line.ends_with('\n') {
                guard.push('\n');
            }
        }
    }

    fn bump_generation(generation: &Arc<Mutex<u64>>) {
        if let Ok(mut gen) = generation.lock() {
            *gen += 1;
        }
    }

    fn submit_prompt(&self) {
        let prompt = self
            .input
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default();
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        if self.busy.lock().map(|b| *b).unwrap_or(false) {
            return;
        }

        *self.busy.lock().expect("busy") = true;
        *self.status.lock().expect("status") = "Cursor 思考中…".to_string();
        Self::append_log(&self.log, &format!("\n> {prompt}\n"));
        Self::bump_generation(&self.generation);

        let node_binary = self.node_binary.clone();
        let cursor_script = self.cursor_script.clone();
        let cursor_workdir = self.cursor_workdir.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let data_dir = self.data_dir.clone();
        let session_key = self.session_key.clone();
        let log = self.log.clone();
        let status = self.status.clone();
        let busy = self.busy.clone();
        let generation = self.generation.clone();
        let resume_id = Self::load_agent_id(&data_dir, &session_key);

        std::thread::spawn(move || {
            let result = Self::run_cursor_turn(
                &node_binary,
                &cursor_script,
                &cursor_workdir,
                &api_key,
                &model,
                &prompt,
                resume_id.as_deref(),
            );
            match result {
                Ok(output) => {
                    if let Some(agent_id) = output
                        .agent_id
                        .as_deref()
                        .filter(|id| Self::is_safe_agent_id(id))
                    {
                        let _ = Self::save_agent_id(&data_dir, &session_key, agent_id);
                    }
                    if let Some(content) = output.content.filter(|c| !c.trim().is_empty()) {
                        Self::append_log(&log, &format!("\n{content}\n"));
                    } else if let Some(err) = output.error {
                        Self::append_log(&log, &format!("\n[error] {err}\n"));
                    }
                    let label = output.status.unwrap_or_else(|| "finished".into());
                    *status.lock().expect("status") = format!("完成 · {label}");
                }
                Err(err) => {
                    Self::append_log(&log, &format!("\n[error] {err}\n"));
                    *status.lock().expect("status") = "失败".to_string();
                }
            }
            *busy.lock().expect("busy") = false;
            Self::bump_generation(&generation);
        });
    }

    fn run_cursor_turn(
        node: &Path,
        script: &Path,
        workdir: &Path,
        api_key: &str,
        model: &str,
        prompt: &str,
        agent_id: Option<&str>,
    ) -> Result<CursorScriptOutput, String> {
        if !script.is_file() {
            return Err(format!("未找到 Cursor 脚本：{}", script.display()));
        }
        if !workdir.is_dir() {
            return Err(format!("工作目录不存在：{}", workdir.display()));
        }
        let desktop_dir = script
            .parent()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| workdir.to_path_buf());

        let payload = CursorScriptInput {
            api_key,
            model,
            cwd: &workdir.display().to_string(),
            prompt,
            agent_id,
        };
        let stdin_json =
            serde_json::to_string(&payload).map_err(|e| format!("serialize request: {e}"))?;

        let mut child = Command::new(node)
            .arg(script)
            .current_dir(&desktop_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn cursor agent: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(stdin_json.as_bytes())
                .map_err(|e| format!("write stdin: {e}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("wait cursor agent: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stdout.is_empty() {
            return Err(if stderr.is_empty() {
                format!("cursor agent produced no output (exit {})", output.status)
            } else {
                stderr
            });
        }
        let parsed: CursorScriptOutput = serde_json::from_str(&stdout)
            .map_err(|e| format!("invalid cursor JSON: {e}; stdout={stdout}"))?;
        if !parsed.ok {
            return Err(parsed.error.unwrap_or_else(|| "cursor agent failed".into()));
        }
        Ok(parsed)
    }

    fn keystroke_to_input(keystroke: &Keystroke, input: &mut String) -> bool {
        let key = keystroke.key.as_str();
        match key {
            "enter" | "return" => return true,
            "backspace" => {
                input.pop();
            }
            "tab" => input.push('\t'),
            _ if key.len() == 1 => {
                if let Some(ch) = key.chars().next() {
                    input.push(ch);
                }
            }
            _ => {}
        }
        false
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
}

impl Entity for CursorAgentView {
    type Event = ();
}

impl View for CursorAgentView {
    fn ui_name() -> &'static str {
        "CursorAgentView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let status = self
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "…".into());
        let log = self.log.lock().map(|s| s.clone()).unwrap_or_default();
        let input = self.input.lock().map(|s| s.clone()).unwrap_or_default();
        let prompt_line = format!("> {input}_");

        let body = ConstrainedBox::new(
            ui_text::mono(log, self.font)
                .with_color(ColorU::new(220, 220, 220, 255))
                .finish(),
        )
        .with_width(VIEW_W)
        .with_height(VIEW_H - INPUT_H - 40.)
        .finish();

        let input_box = Container::new(
            ui_text::body(prompt_line, self.font)
                .with_color(ColorU::new(140, 200, 255, 255))
                .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        let header = Container::new(
            ui_text::body(status, self.font)
                .with_color(ColorU::new(180, 180, 180, 255))
                .finish(),
        )
        .with_uniform_padding(8.)
        .finish();

        let input_handler = EventHandler::new(input_box)
            .with_always_handle()
            .on_keydown({
                let view = self.clone_for_action();
                move |_, _, keystroke| {
                    view.handle_keystroke(keystroke);
                    DispatchEventResult::StopPropagation
                }
            })
            .finish();

        Flex::column()
            .with_child(header)
            .with_child(body)
            .with_child(input_handler)
            .finish()
    }
}

impl CursorAgentView {
    fn clone_for_action(&self) -> CursorAgentViewHandle {
        CursorAgentViewHandle {
            input: self.input.clone(),
            generation: self.generation.clone(),
            submit: CursorAgentSubmit {
                node_binary: self.node_binary.clone(),
                cursor_script: self.cursor_script.clone(),
                cursor_workdir: self.cursor_workdir.clone(),
                api_key: self.api_key.clone(),
                model: self.model.clone(),
                data_dir: self.data_dir.clone(),
                session_key: self.session_key.clone(),
                log: self.log.clone(),
                status: self.status.clone(),
                busy: self.busy.clone(),
                generation: self.generation.clone(),
            },
        }
    }

    fn handle_keystroke(&self, keystroke: &Keystroke) {
        let submit = {
            let mut input = self.input.lock().expect("input");
            Self::keystroke_to_input(keystroke, &mut input)
        };
        Self::bump_generation(&self.generation);
        if submit {
            self.submit_prompt();
        }
    }
}

#[derive(Clone)]
struct CursorAgentSubmit {
    node_binary: PathBuf,
    cursor_script: PathBuf,
    cursor_workdir: PathBuf,
    api_key: String,
    model: String,
    data_dir: PathBuf,
    session_key: String,
    log: Arc<Mutex<String>>,
    status: Arc<Mutex<String>>,
    busy: Arc<Mutex<bool>>,
    generation: Arc<Mutex<u64>>,
}

#[derive(Clone)]
struct CursorAgentViewHandle {
    input: Arc<Mutex<String>>,
    generation: Arc<Mutex<u64>>,
    submit: CursorAgentSubmit,
}

impl CursorAgentViewHandle {
    fn handle_keystroke(&self, keystroke: &Keystroke) {
        let submit = {
            let mut input = self.input.lock().expect("input");
            CursorAgentView::keystroke_to_input(keystroke, &mut input)
        };
        CursorAgentView::bump_generation(&self.generation);
        if submit {
            let prompt = self
                .input
                .lock()
                .map(|mut g| std::mem::take(&mut *g))
                .unwrap_or_default();
            let prompt = prompt.trim().to_string();
            if prompt.is_empty() {
                return;
            }
            if self.submit.busy.lock().map(|b| *b).unwrap_or(false) {
                return;
            }
            *self.submit.busy.lock().expect("busy") = true;
            *self.submit.status.lock().expect("status") = "Cursor 思考中…".to_string();
            CursorAgentView::append_log(&self.submit.log, &format!("\n> {prompt}\n"));
            CursorAgentView::bump_generation(&self.submit.generation);

            let s = self.submit.clone();
            std::thread::spawn(move || {
                let resume_id = CursorAgentView::load_agent_id(&s.data_dir, &s.session_key);
                let result = CursorAgentView::run_cursor_turn(
                    &s.node_binary,
                    &s.cursor_script,
                    &s.cursor_workdir,
                    &s.api_key,
                    &s.model,
                    &prompt,
                    resume_id.as_deref(),
                );
                match result {
                    Ok(output) => {
                        if let Some(agent_id) = output
                            .agent_id
                            .as_deref()
                            .filter(|id| CursorAgentView::is_safe_agent_id(id))
                        {
                            let _ = CursorAgentView::save_agent_id(
                                &s.data_dir,
                                &s.session_key,
                                agent_id,
                            );
                        }
                        if let Some(content) = output.content.filter(|c| !c.trim().is_empty()) {
                            CursorAgentView::append_log(&s.log, &format!("\n{content}\n"));
                        } else if let Some(err) = output.error {
                            CursorAgentView::append_log(&s.log, &format!("\n[error] {err}\n"));
                        }
                        let label = output.status.unwrap_or_else(|| "finished".into());
                        *s.status.lock().expect("status") = format!("完成 · {label}");
                    }
                    Err(err) => {
                        CursorAgentView::append_log(&s.log, &format!("\n[error] {err}\n"));
                        *s.status.lock().expect("status") = "失败".to_string();
                    }
                }
                *s.busy.lock().expect("busy") = false;
                CursorAgentView::bump_generation(&s.generation);
            });
        }
    }
}

impl TypedActionView for CursorAgentView {
    type Action = CursorAgentAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CursorAgentAction::Keydown(keystroke) => {
                self.handle_keystroke(keystroke);
                ctx.notify();
            }
        }
    }
}
