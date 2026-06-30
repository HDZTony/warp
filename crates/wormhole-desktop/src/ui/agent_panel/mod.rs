mod composer_menus;
mod sidebar;
mod transcript;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use transcript::{render_transcript, TranscriptLine, TranscriptViewModel};
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use composer_menus::{access_label, model_label};
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ConstrainedBox, Container, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Shrinkable,
    Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::keymap::Keystroke;
use wormhole_desktop_core::agent_llm_commands::{AgentLlmChatMessage, AgentLlmChatParams};
use wormhole_desktop_core::warp_embed_prefs::{self, AgentAccessMode, PreferredAgent};
use wormhole_desktop_core::{
    agent_codex_chat_with_stream, agent_cursor_chat, agent_cursor_start_session,
    agent_launch_codex_terminal, agent_launch_cursor_terminal, agent_read_local_session_events,
    agent_start_session, agent_status, AgentStartRequest,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::multiline_input;
use crate::ui::panel_primitives::{agent_header_bg, tab_content_fill, AGENT_THREAD_BOTTOM_PAD, AGENT_THREAD_MAX_WIDTH};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, CaretBlink, CaretBlinkHost, TextFieldEditAction,
    TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

const COMPOSER_BAR_LIFT: f32 = 44.0;
const ACCESS_POPOVER_INSET_LEFT: f32 = 46.0;
const MODEL_POPOVER_INSET_RIGHT: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Chat,
    Task,
}

#[derive(Debug, Clone)]
pub enum AgentPanelAction {
    SetVisible(bool),
    SelectAgent(PreferredAgent),
    SelectAccessMode(AgentAccessMode),
    ToggleAccessMenu,
    ToggleModelMenu,
    DismissComposerMenus,
    SelectMode(InteractionMode),
    Send,
    PasteInput,
    FocusInput,
    NewConversation,
    LaunchTerminal,
    RefreshStatus,
    SelectProject(String),
    SelectSession(String),
    NewProject,
    NewThread,
    FocusSidebarSearch,
    SidebarMore,
    Stop,
    TextFieldEdit(TextFieldEditAction),
}

struct PanelState {
    status: String,
    daemon_running: bool,
    draft: String,
    busy: bool,
    mode: InteractionMode,
    lines: Arc<Vec<TranscriptLine>>,
    chat_messages: Vec<AgentLlmChatMessage>,
    resume_id: Option<String>,
    active_session_id: Option<String>,
    event_cursor: u64,
    polling_session: bool,
    input_focused: bool,
    pending_send: bool,
    projects: Vec<sidebar::AgentProject>,
    sidebar_sessions: Vec<sidebar::AgentSession>,
    active_project_id: String,
    active_sidebar_session_id: String,
    thread_title: String,
    sidebar_search: String,
    sidebar_search_focused: bool,
    demo_user_prompt: Option<String>,
    demo_status_line: Option<String>,
    demo_assistant_body: Option<String>,
    demo_thinking: bool,
    access_menu_open: bool,
    model_menu_open: bool,
    session_running: bool,
    field_state: TextFieldState,
}

pub struct AgentPanelView {
    core: CoreHandle,
    font: FamilyId,
    mono: FamilyId,
    state: Arc<Mutex<PanelState>>,
    generation: Arc<Mutex<u64>>,
    event_poll_inflight: Arc<AtomicBool>,
    visible: bool,
    preferred_agent: PreferredAgent,
    access_mode: AgentAccessMode,
    sidebar_scroll: ClippedScrollStateHandle,
    caret_blink: CaretBlink,
    generation_notify_tx: async_channel::Sender<()>,
    generation_notify_rx: async_channel::Receiver<()>,
}

impl AgentPanelView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let prefs = warp_embed_prefs::load_prefs(&core.data_dir());
        let preferred_agent = prefs.preferred_agent;
        let access_mode = prefs.access_mode;
        let (generation_notify_tx, generation_notify_rx) = async_channel::unbounded();
        Self {
            core,
            font,
            mono,
            state: Arc::new(Mutex::new(PanelState {
                status: "正在检查 wormhole-agentd…".to_string(),
                daemon_running: false,
                draft: String::new(),
                busy: false,
                mode: InteractionMode::Chat,
                lines: Arc::new(Vec::new()),
                chat_messages: Vec::new(),
                resume_id: None,
                active_session_id: None,
                event_cursor: 0,
                polling_session: false,
                input_focused: false,
                pending_send: false,
                projects: sidebar::seed_projects(),
                sidebar_sessions: sidebar::seed_sessions(),
                active_project_id: "wormhole".into(),
                active_sidebar_session_id: "sync-share".into(),
                thread_title: "同步终端共享文件夹".into(),
                sidebar_search: String::new(),
                sidebar_search_focused: false,
                demo_user_prompt: Some(
                    "扫描三台终端的共享文件夹，把未同步的 Specs 文档全部拉取到本机。".into(),
                ),
                demo_status_line: Some("已运行 19 秒".into()),
                demo_assistant_body: Some(
                    sidebar::seed_sessions()
                        .into_iter()
                        .find(|s| s.id == "sync-share")
                        .map(|s| s.body)
                        .unwrap_or_default(),
                ),
                demo_thinking: true,
                access_menu_open: false,
                model_menu_open: false,
                session_running: true,
                field_state: TextFieldState::new(),
            })),
            generation: Arc::new(Mutex::new(0)),
            event_poll_inflight: Arc::new(AtomicBool::new(false)),
            visible: false,
            preferred_agent,
            access_mode,
            sidebar_scroll: ClippedScrollStateHandle::default(),
            caret_blink: CaretBlink::new(),
            generation_notify_tx,
            generation_notify_rx,
        }
    }

    pub fn focus_with_agent(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        self.preferred_agent = agent;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = core
                .block_on(async { warp_embed_prefs::set_preferred_agent(&data_dir, agent).await });
        });
        if !self.visible {
            self.set_tab_visible(true, ctx);
        } else {
            self.refresh_status(ctx);
        }
        ctx.notify();
    }

    pub fn set_tab_visible(&mut self, visible: bool, ctx: &mut ViewContext<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            if let Ok(mut panel) = self.state.lock() {
                panel.input_focused = true;
                panel.sidebar_search_focused = false;
            }
            self.refresh_status(ctx);
            self.start_generation_listener(ctx);
            sync_caret_blink(self, ctx);
        } else {
            sync_caret_blink(self, ctx);
        }
        ctx.notify();
    }

    fn notify_generation(&self) {
        let _ = self.generation_notify_tx.try_send(());
    }

    fn bump(&self) {
        if let Ok(mut gen) = self.generation.lock() {
            *gen = gen.saturating_add(1);
        }
        self.notify_generation();
    }

    fn start_session_poll(&self, ctx: &mut ViewContext<Self>) {
        let state = Arc::clone(&self.state);
        let core = self.core.clone();
        let inflight = Arc::clone(&self.event_poll_inflight);
        let notify_tx = self.generation_notify_tx.clone();
        Self::session_poll_once(ctx, state, core, inflight, notify_tx);
    }

    fn session_poll_once(
        ctx: &mut ViewContext<Self>,
        state: Arc<Mutex<PanelState>>,
        core: CoreHandle,
        inflight: Arc<AtomicBool>,
        notify_tx: async_channel::Sender<()>,
    ) {
        let should_poll = state
            .lock()
            .ok()
            .map(|panel| panel.polling_session && panel.active_session_id.is_some())
            .unwrap_or(false);
        if !should_poll {
            return;
        }

        if !inflight.swap(true, Ordering::SeqCst) {
            let (session_id, cursor) = {
                let panel = state.lock().expect("agent panel state");
                (
                    panel.active_session_id.clone().expect("session id"),
                    panel.event_cursor,
                )
            };
            let state_for_task = Arc::clone(&state);
            let inflight_for_task = Arc::clone(&inflight);
            let notify_for_task = notify_tx.clone();
            let core_for_async = core.clone();
            let core_for_poll = core.clone();
            ctx.spawn(
                async move {
                    agent_read_local_session_events(session_id, cursor, core_for_async.app_state())
                        .await
                },
                move |view, output, ctx| {
                    inflight_for_task.store(false, Ordering::SeqCst);
                    if let Ok(page) = output {
                        let mut panel = state_for_task.lock().expect("agent panel state");
                        for event in page.events {
                            Self::push_line(
                                &mut panel,
                                TranscriptLine {
                                    channel: event.channel,
                                    text: event.text,
                                    level: event.level,
                                },
                            );
                        }
                        panel.event_cursor = page.next_cursor;
                        if matches!(page.status.as_str(), "completed" | "failed") {
                            panel.polling_session = false;
                            panel.busy = false;
                            panel.active_session_id = None;
                            panel.status = format!("任务{}", page.status);
                        }
                    }
                    let _ = notify_for_task.try_send(());
                    ctx.notify();
                    if view.visible {
                        Self::session_poll_once(
                            ctx,
                            state_for_task,
                            core_for_poll,
                            inflight_for_task,
                            notify_for_task,
                        );
                    }
                },
            );
            return;
        }

        ctx.spawn(
            async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
            },
            move |view, _, ctx| {
                if view.visible {
                    Self::session_poll_once(ctx, state, core, inflight, notify_tx);
                }
            },
        );
    }

    fn start_generation_listener(&self, ctx: &mut ViewContext<Self>) {
        let rx = self.generation_notify_rx.clone();
        let state = Arc::clone(&self.state);
        Self::generation_notify_once(ctx, rx, state);
    }

    fn generation_notify_once(
        ctx: &mut ViewContext<Self>,
        rx: async_channel::Receiver<()>,
        state: Arc<Mutex<PanelState>>,
    ) {
        let waiter = rx.clone();
        let state_for_task = Arc::clone(&state);
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    let should_send = state_for_task
                        .lock()
                        .map(|mut panel| {
                            if panel.pending_send {
                                panel.pending_send = false;
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if should_send {
                        view.send_message(ctx);
                    } else {
                        sync_caret_blink(view, ctx);
                        ctx.notify();
                    }
                    if view.visible {
                        Self::generation_notify_once(ctx, rx, state_for_task);
                    }
                }
            },
        );
    }

    fn push_line(state: &mut PanelState, line: TranscriptLine) {
        let lines = Arc::make_mut(&mut state.lines);
        lines.push(line);
    }

    fn keystroke_action(
        state: &Arc<Mutex<PanelState>>,
        preferred_agent: PreferredAgent,
        keystroke: &Keystroke,
    ) -> Option<AgentPanelAction> {
        if keystroke.ctrl || keystroke.meta || keystroke.alt {
            return None;
        }
        let panel = state.lock().ok()?;
        if panel.input_focused || panel.busy {
            return None;
        }
        match keystroke.key.as_str() {
            "left" | "right" => {
                let agent = if preferred_agent == PreferredAgent::Codex {
                    PreferredAgent::Cursor
                } else {
                    PreferredAgent::Codex
                };
                Some(AgentPanelAction::SelectAgent(agent))
            }
            "up" | "down" => {
                let mode = if panel.mode == InteractionMode::Chat {
                    InteractionMode::Task
                } else {
                    InteractionMode::Chat
                };
                Some(AgentPanelAction::SelectMode(mode))
            }
            _ => None,
        }
    }

    fn handle_keystroke_panel(
        state: &Arc<Mutex<PanelState>>,
        keystroke: &Keystroke,
        notify_tx: &async_channel::Sender<()>,
    ) -> bool {
        if keystroke.key == "tab" {
            if let Ok(mut panel) = state.lock() {
                if panel.sidebar_search_focused {
                    panel.sidebar_search_focused = false;
                    panel.input_focused = true;
                } else {
                    panel.input_focused = !panel.input_focused;
                    panel.sidebar_search_focused = false;
                }
                if panel.input_focused {
                    panel.sidebar_search_focused = false;
                }
            }
            let _ = notify_tx.try_send(());
            return true;
        }
        let mut panel = state.lock().expect("agent panel state");
        if panel.sidebar_search_focused {
            match keystroke.key.as_str() {
                "backspace" => {
                    panel.sidebar_search.pop();
                }
                "escape" => {
                    panel.sidebar_search.clear();
                    panel.sidebar_search_focused = false;
                }
                key if key.len() == 1 => {
                    if let Some(ch) = key.chars().next() {
                        if !keystroke.ctrl && !keystroke.meta {
                            panel.sidebar_search.push(ch);
                        }
                    }
                }
                _ => return false,
            }
            drop(panel);
            let _ = notify_tx.try_send(());
            return true;
        }
        if !panel.busy && !panel.input_focused {
            return false;
        }
        if !panel.input_focused || panel.busy {
            return false;
        }
        match keystroke.key.as_str() {
            "enter" | "return" => {
                if keystroke.shift {
                    panel.draft.push('\n');
                    panel.field_state.clear_marked();
                } else if !panel.draft.trim().is_empty() {
                    panel.pending_send = true;
                }
            }
            "escape" => {
                panel.draft.clear();
                panel.field_state.clear_marked();
                panel.input_focused = false;
            }
            "backspace" => return false,
            key if key.len() == 1 => return false,
            _ => return false,
        }
        drop(panel);
        let _ = notify_tx.try_send(());
        true
    }

    fn select_project(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        let first = {
            let mut panel = self.state.lock().expect("agent panel state");
            if panel.active_project_id == project_id {
                return;
            }
            panel.active_project_id = project_id.clone();
            panel
                .sidebar_sessions
                .iter()
                .find(|s| s.project_id == project_id)
                .map(|s| s.id.clone())
        };
        if let Some(session_id) = first {
            self.select_session(session_id, ctx);
        } else {
            ctx.notify();
        }
    }

    fn select_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        let snapshot = {
            let panel = self.state.lock().expect("agent panel state");
            panel
                .sidebar_sessions
                .iter()
                .find(|s| s.id == session_id)
                .cloned()
        };
        let mut panel = self.state.lock().expect("agent panel state");
        panel.active_sidebar_session_id = session_id;
        panel.lines = Arc::new(Vec::new());
        if let Some(session) = snapshot {
            panel.thread_title = session.label.clone();
            panel.session_running = session.running;
            panel.demo_user_prompt = Some(session.prompt.clone());
            panel.demo_assistant_body = Some(session.body.clone());
            panel.demo_status_line = Some(if session.running {
                "已运行 19 秒".into()
            } else {
                "已完成".into()
            });
            panel.demo_thinking = session.running;
            panel.busy = session.running;
        }
        drop(panel);
        self.bump();
        ctx.notify();
    }

    fn new_project(&mut self, ctx: &mut ViewContext<Self>) {
        let mut panel = self.state.lock().expect("agent panel state");
        let n = panel.projects.len() + 1;
        let id = format!("project-{n}");
        panel.projects.push(sidebar::AgentProject {
            id: id.clone(),
            label: format!("新项目 {n}"),
            time: "now".into(),
        });
        panel.active_project_id = id;
        drop(panel);
        ctx.notify();
    }

    fn new_thread(&mut self, ctx: &mut ViewContext<Self>) {
        let mut panel = self.state.lock().expect("agent panel state");
        let n = panel
            .sidebar_sessions
            .iter()
            .filter(|s| s.project_id == panel.active_project_id)
            .count()
            + 1;
        let id = format!("thread-{n}");
        let project_id = panel.active_project_id.clone();
        panel.sidebar_sessions.insert(
            0,
            sidebar::AgentSession {
                id: id.clone(),
                label: format!("新会话 {n}"),
                project_id,
                time: "now".into(),
                running: false,
                model: "GPT-5.5".into(),
                prompt: String::new(),
                body: String::new(),
            },
        );
        panel.active_sidebar_session_id = id.clone();
        panel.thread_title = format!("新会话 {n}");
        panel.lines = Arc::new(Vec::new());
        panel.demo_user_prompt = None;
        panel.demo_status_line = None;
        panel.demo_assistant_body = None;
        panel.demo_thinking = false;
        panel.session_running = false;
        drop(panel);
        self.bump();
        ctx.notify();
    }

    fn dismiss_composer_menus(&mut self, ctx: &mut ViewContext<Self>) {
        let mut changed = false;
        if let Ok(mut panel) = self.state.lock() {
            if panel.access_menu_open || panel.model_menu_open {
                panel.access_menu_open = false;
                panel.model_menu_open = false;
                changed = true;
            }
        }
        if changed {
            ctx.notify();
        }
    }

    fn toggle_access_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.model_menu_open = false;
            panel.access_menu_open = !panel.access_menu_open;
        }
        ctx.notify();
    }

    fn toggle_model_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.access_menu_open = false;
            panel.model_menu_open = !panel.model_menu_open;
        }
        ctx.notify();
    }

    fn select_access_mode(&mut self, mode: AgentAccessMode, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.access_menu_open = false;
            panel.model_menu_open = false;
        }
        if self.access_mode == mode {
            ctx.notify();
            return;
        }
        self.access_mode = mode;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = core.block_on(async { warp_embed_prefs::set_access_mode(&data_dir, mode).await });
        });
        ctx.notify();
    }

    fn select_agent(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.model_menu_open = false;
            panel.access_menu_open = false;
        }
        if self.preferred_agent != agent {
            self.preferred_agent = agent;
            if let Ok(mut panel) = self.state.lock() {
                panel.resume_id = None;
            }
            let data_dir = self.core.data_dir();
            let core = self.core.clone();
            std::thread::spawn(move || {
                let _ = core.block_on(async {
                    warp_embed_prefs::set_preferred_agent(&data_dir, agent).await
                });
            });
            self.update_status_line(ctx);
        }
        ctx.notify();
    }

    fn select_mode(&mut self, mode: InteractionMode, ctx: &mut ViewContext<Self>) {
        let mut state = self.state.lock().expect("agent panel state");
        state.mode = mode;
        drop(state);
        ctx.notify();
    }

    fn new_conversation(&mut self, ctx: &mut ViewContext<Self>) {
        {
            let mut state = self.state.lock().expect("agent panel state");
            state.lines = Arc::new(Vec::new());
            state.chat_messages.clear();
            state.resume_id = None;
            state.active_session_id = None;
            state.event_cursor = 0;
            state.polling_session = false;
            state.busy = false;
            state.draft.clear();
            state.status = "已开启新对话".into();
            state.demo_user_prompt = None;
            state.demo_status_line = None;
            state.demo_assistant_body = None;
            state.demo_thinking = false;
            state.session_running = false;
        }
        self.bump();
        ctx.notify();
    }

    fn update_status_line(&self, ctx: &mut ViewContext<Self>) {
        let agent_label = match self.preferred_agent {
            PreferredAgent::Codex => "Codex",
            PreferredAgent::Cursor => "Cursor",
        };
        let mut state = self.state.lock().expect("agent panel state");
        state.status = if state.daemon_running {
            format!(
                "wormhole-agentd 运行中 · {agent_label} · {}",
                match state.mode {
                    InteractionMode::Chat => "对话模式",
                    InteractionMode::Task => "全自动模式",
                }
            )
        } else {
            format!("wormhole-agentd 未就绪 · {agent_label}（远程 iOS 任务需要 daemon）")
        };
        ctx.notify();
    }

    fn refresh_status(&self, ctx: &mut ViewContext<Self>) {
        let shared = Arc::clone(&self.state);
        let core = self.core.clone();
        ctx.spawn(
            async move { agent_status(core.app_state()).await },
            move |view, output, ctx| {
                if let Ok(snapshot) = output {
                    let mut state = shared.lock().expect("agent panel state");
                    state.daemon_running = snapshot.daemon_running;
                }
                view.update_status_line(ctx);
            },
        );
    }

    fn send_message(&mut self, ctx: &mut ViewContext<Self>) {
        let (prompt, mode) = {
            let mut state = self.state.lock().expect("agent panel state");
            if state.busy {
                return;
            }
            let text = state.draft.trim().to_string();
            if text.is_empty() {
                return;
            }
            state.busy = true;
            state.draft.clear();
            Self::push_line(
                &mut state,
                TranscriptLine {
                    channel: "user".into(),
                    text: text.clone(),
                    level: "info".into(),
                },
            );
            state.chat_messages.push(AgentLlmChatMessage {
                role: "user".into(),
                content: text.clone(),
            });
            state.demo_user_prompt = None;
            state.demo_status_line = None;
            state.demo_assistant_body = None;
            state.demo_thinking = false;
            state.session_running = false;
            state.status = "执行中…".into();
            (text, state.mode)
        };
        self.bump();
        ctx.notify();
        match mode {
            InteractionMode::Chat => self.send_chat(ctx),
            InteractionMode::Task => self.send_task(ctx, prompt),
        }
    }

    fn send_chat(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let shared_async = Arc::clone(&self.state);
        let shared_callback = Arc::clone(&self.state);
        let generation_callback = Arc::clone(&self.generation);
        let agent = self.preferred_agent;
        let access_mode = self.access_mode;
        let generation_notify = self.generation_notify_tx.clone();
        ctx.spawn(
            async move {
                let params = {
                    let state = shared_async.lock().expect("agent panel state");
                    let mut params = AgentLlmChatParams {
                        messages: state.chat_messages.clone(),
                        cwd: None,
                        thread_id: None,
                        agent_id: None,
                        force_full_auto: access_mode.force_full_auto(),
                    };
                    let resume = state.resume_id.clone();
                    match agent {
                        PreferredAgent::Codex => params.thread_id = resume,
                        PreferredAgent::Cursor => {
                            params.agent_id = resume.clone();
                            params.thread_id = resume;
                        }
                    }
                    params
                };
                let state = core.app_state();
                match agent {
                    PreferredAgent::Codex => {
                        let stream_shared = Arc::clone(&shared_async);
                        let stream_notify = generation_notify.clone();
                        agent_codex_chat_with_stream(params, state, move |event| {
                            let level = if event.channel == "error" {
                                "error".to_string()
                            } else {
                                "info".to_string()
                            };
                            if let Ok(mut panel) = stream_shared.lock() {
                                Self::push_line(
                                    &mut panel,
                                    TranscriptLine {
                                        channel: event.channel,
                                        text: event.text,
                                        level,
                                    },
                                );
                            }
                            let _ = stream_notify.try_send(());
                        })
                        .await
                    }
                    PreferredAgent::Cursor => agent_cursor_chat(params, state).await,
                }
            },
            move |_view, output, ctx| {
                let mut panel = shared_callback.lock().expect("agent panel state");
                match output {
                    Ok(reply) => {
                        Self::push_line(
                            &mut panel,
                            TranscriptLine {
                                channel: "assistant".into(),
                                text: reply.content.clone(),
                                level: "info".into(),
                            },
                        );
                        panel.chat_messages.push(AgentLlmChatMessage {
                            role: "assistant".into(),
                            content: reply.content,
                        });
                        panel.resume_id = reply.thread_id;
                        panel.status = format!("完成 · {}", reply.model);
                    }
                    Err(err) => {
                        Self::push_line(
                            &mut panel,
                            TranscriptLine {
                                channel: "stderr".into(),
                                text: err.clone(),
                                level: "error".into(),
                            },
                        );
                        panel.status = err;
                    }
                }
                panel.busy = false;
                drop(panel);
                if let Ok(mut gen) = generation_callback.lock() {
                    *gen = gen.saturating_add(1);
                }
                ctx.notify();
            },
        );
    }

    fn send_task(&mut self, ctx: &mut ViewContext<Self>, prompt: String) {
        let core = self.core.clone();
        let shared = Arc::clone(&self.state);
        let generation = Arc::clone(&self.generation);
        let agent = self.preferred_agent;
        ctx.spawn(
            async move {
                let request = AgentStartRequest { prompt, cwd: None };
                let state = core.app_state();
                match agent {
                    PreferredAgent::Codex => agent_start_session(request, state).await,
                    PreferredAgent::Cursor => agent_cursor_start_session(request, state).await,
                }
            },
            move |view, output, ctx| {
                let mut panel = shared.lock().expect("agent panel state");
                let mut started = false;
                match output {
                    Ok(session) => {
                        Self::push_line(
                            &mut panel,
                            TranscriptLine {
                                channel: "status".into(),
                                text: format!("任务已启动 · session {}", session.id),
                                level: "info".into(),
                            },
                        );
                        panel.active_session_id = Some(session.id);
                        panel.event_cursor = 0;
                        panel.polling_session = true;
                        panel.status = "任务运行中…".into();
                        started = true;
                    }
                    Err(err) => {
                        Self::push_line(
                            &mut panel,
                            TranscriptLine {
                                channel: "stderr".into(),
                                text: err.clone(),
                                level: "error".into(),
                            },
                        );
                        panel.status = err;
                        panel.busy = false;
                    }
                }
                if panel.active_session_id.is_none() {
                    panel.busy = false;
                }
                drop(panel);
                if let Ok(mut gen) = generation.lock() {
                    *gen = gen.saturating_add(1);
                }
                ctx.notify();
                if started {
                    view.start_session_poll(ctx);
                }
            },
        );
    }

    fn launch_terminal(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let agent = self.preferred_agent;
        let shared = Arc::clone(&self.state);
        ctx.spawn(
            async move {
                let app = core.runtime().ctx.clone();
                let state = core.app_state();
                match agent {
                    PreferredAgent::Codex => agent_launch_codex_terminal(&app, None, state).await,
                    PreferredAgent::Cursor => agent_launch_cursor_terminal(&app, None, state).await,
                }
            },
            move |_view, output, ctx| {
                if let Err(err) = output {
                    let mut state = shared.lock().expect("agent panel state");
                    state.status = format!("无法启动终端: {err}");
                    ctx.notify();
                }
            },
        );
    }

    fn paste_input(&mut self, ctx: &mut ViewContext<Self>) {
        let text = arboard::Clipboard::new()
            .and_then(|mut clip| clip.get_text())
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            return;
        }
        {
            let mut state = self.state.lock().expect("agent panel state");
            if state.busy {
                return;
            }
            state.draft = text;
        }
        self.bump();
        ctx.notify();
    }

    fn segmented(
        &self,
        labels: &[(&str, bool)],
        action: impl Fn(usize) -> AgentPanelAction,
    ) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        for (idx, (label, selected)) in labels.iter().enumerate() {
            let bg = if *selected {
                theme::accent_bg(48)
            } else {
                ColorU::new(0, 0, 0, 0)
            };
            let color = if *selected {
                theme::accent()
            } else {
                theme::text()
            };
            let action_label = action(idx);
            let label_el = ui_text::body(label.to_string(), self.font)
                .with_color(color)
                .finish();
            let btn = Container::new(
                EventHandler::new(label_el)
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(action_label.clone());
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_uniform_padding(12.0)
            .with_background(bg)
            .finish();
            row.add_child(btn);
        }
        Container::new(row.finish())
            .with_uniform_padding(4.0)
            .with_background(theme::bg())
            .finish()
    }

    fn agent_switcher(&self) -> Box<dyn Element> {
        let selected_codex = self.preferred_agent == PreferredAgent::Codex;
        self.segmented(
            &[("Codex", selected_codex), ("Cursor", !selected_codex)],
            |idx| {
                AgentPanelAction::SelectAgent(if idx == 0 {
                    PreferredAgent::Codex
                } else {
                    PreferredAgent::Cursor
                })
            },
        )
    }

    fn mode_switcher(&self) -> Box<dyn Element> {
        let mode = self.state.lock().expect("agent panel state").mode;
        let chat = mode == InteractionMode::Chat;
        self.segmented(&[("对话", chat), ("全自动", !chat)], |idx| {
            AgentPanelAction::SelectMode(if idx == 0 {
                InteractionMode::Chat
            } else {
                InteractionMode::Task
            })
        })
    }

    fn toolbar_button(
        &self,
        label: &str,
        action: AgentPanelAction,
        primary: bool,
    ) -> Box<dyn Element> {
        let (fg, bg) = if primary {
            (theme::accent(), theme::accent_bg(40))
        } else {
            (theme::text(), ColorU::new(0, 0, 0, 0))
        };
        let label_el = ui_text::body(label.to_string(), self.font)
            .with_color(fg)
            .finish();
        Container::new(
            EventHandler::new(label_el)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_uniform_padding(12.0)
        .with_background(bg)
        .finish()
    }

    fn composer_chip(
        &self,
        label: impl Into<String>,
        action: Option<AgentPanelAction>,
        accent: bool,
    ) -> Box<dyn Element> {
        let label = label.into();
        let text = ui_text::body(label, self.font)
            .with_color(if accent {
                theme::text()
            } else {
                theme::muted()
            })
            .finish();
        let inner = if let Some(action) = action {
            EventHandler::new(text)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            text
        };
        Container::new(inner)
            .with_uniform_padding(8.0)
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(8.0),
            ))
            .finish()
    }

    fn composer_icon_chip(
        &self,
        path: &'static str,
        color: ColorU,
        action: Option<AgentPanelAction>,
    ) -> Box<dyn Element> {
        let inner = icons::agent_composer_icon(path, color);
        let wrapped: Box<dyn Element> = if let Some(action) = action {
            EventHandler::new(inner)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            inner
        };
        Container::new(wrapped)
            .with_uniform_padding(8.0)
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(8.0),
            ))
            .finish()
    }

    fn composer_labeled_chip(
        &self,
        label: impl Into<String>,
        action: Option<AgentPanelAction>,
        leading_icon: Option<&'static str>,
        trailing_chevron: bool,
    ) -> Box<dyn Element> {
        let label = label.into();
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        if let Some(path) = leading_icon {
            row.add_child(
                Container::new(icons::agent_composer_icon(path, theme::warn()))
                    .with_horizontal_margin(4.0)
                    .finish(),
            );
        }
        row.add_child(
            ui_text::body(label, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        if trailing_chevron {
            row.add_child(
                Container::new(icons::agent_icon("agent-chevron.svg", theme::muted()))
                    .with_horizontal_margin(4.0)
                    .finish(),
            );
        }
        let inner: Box<dyn Element> = if let Some(action) = action {
            EventHandler::new(row.finish())
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish()
        } else {
            row.finish()
        };
        Container::new(inner)
            .with_uniform_padding(8.0)
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(8.0),
            ))
            .finish()
    }

    fn composer_bar(&self, busy: bool, access_mode: AgentAccessMode) -> Box<dyn Element> {
        let access_chip = self.composer_labeled_chip(
            access_label(access_mode),
            Some(AgentPanelAction::ToggleAccessMenu),
            Some("agent-warn.svg"),
            true,
        );
        let model_chip = self.composer_labeled_chip(
            model_label(self.preferred_agent),
            Some(AgentPanelAction::ToggleModelMenu),
            None,
            true,
        );
        let bar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(self.composer_icon_chip("agent-attach.svg", theme::muted(), None))
            .with_child(access_chip)
            .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
            .with_child(model_chip)
            .with_child(self.stop_button(busy));
        Container::new(bar.finish())
            .with_padding_left(10.0)
            .with_padding_right(10.0)
            .with_padding_top(6.0)
            .with_padding_bottom(10.0)
            .finish()
    }

    fn agent_composer_input(
        &self,
        draft: &str,
        marked: &str,
        input_focused: bool,
        caret_blink: bool,
        busy: bool,
        placeholder: &str,
    ) -> Box<dyn Element> {
        let field = render_field_with_caret(
            draft,
            marked,
            placeholder,
            self.font,
            input_focused,
            busy,
            caret_blink,
        );

        TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(AgentPanelAction::TextFieldEdit(action));
        })
        .focused(input_focused)
        .disabled(busy)
        .on_keydown(move |ctx, keystroke| {
            if busy {
                return DispatchEventResult::PropagateToParent;
            }
            match keystroke.key.as_str() {
                "enter" | "return" | "escape" | "tab" => DispatchEventResult::PropagateToParent,
                _ => DispatchEventResult::PropagateToParent,
            }
        })
        .finish()
    }

    fn wrap_composer_with_popovers(
        &self,
        composer: Box<dyn Element>,
        access_mode: AgentAccessMode,
        access_menu_open: bool,
        model_menu_open: bool,
    ) -> Box<dyn Element> {
        if !access_menu_open && !model_menu_open {
            return composer;
        }
        let mut stack = Stack::new();
        stack.add_child(composer);
        if access_menu_open {
            stack.add_child(
                Align::new(
                    Container::new(composer_menus::render_access_menu(self.font, access_mode))
                        .with_margin_left(ACCESS_POPOVER_INSET_LEFT)
                        .with_margin_bottom(COMPOSER_BAR_LIFT)
                        .finish(),
                )
                .bottom_left()
                .finish(),
            );
        }
        if model_menu_open {
            stack.add_child(
                Align::new(
                    Container::new(composer_menus::render_model_menu(
                        self.font,
                        self.preferred_agent,
                    ))
                    .with_margin_right(MODEL_POPOVER_INSET_RIGHT)
                    .with_margin_bottom(COMPOSER_BAR_LIFT)
                    .finish(),
                )
                .bottom_right()
                .finish(),
            );
        }
        stack.finish()
    }

    fn composer_menu_scrim(&self) -> Box<dyn Element> {
        EventHandler::new(
            Container::new(Flex::row().finish())
                .with_background(ColorU::new(8, 7, 11, 150))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::DismissComposerMenus);
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn stop_button(&self, busy: bool) -> Box<dyn Element> {
        let (bg, border, icon_color) = if busy {
            (theme::accent(), ColorU::new(0, 0, 0, 0), theme::canvas())
        } else {
            (theme::panel_elevated(), theme::border(), theme::muted())
        };
        let stop_icon = Container::new(
            ConstrainedBox::new(Flex::row().finish())
                .with_width(10.0)
                .with_height(10.0)
                .finish(),
        )
        .with_background(icon_color)
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(2.0),
        ))
        .finish();
        let mut btn = Container::new(
            ConstrainedBox::new(
                Align::new(stop_icon)
                    .finish(),
            )
            .with_width(32.0)
            .with_height(32.0)
            .finish(),
        )
        .with_background(bg)
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(999.0),
        ));
        if !busy {
            btn = btn.with_border(Border::all(1.0).with_border_fill(border));
        }
        Container::new(
            EventHandler::new(btn.finish())
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(AgentPanelAction::Stop);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_horizontal_margin(4.0)
        .finish()
    }

    fn agent_header(&self, thread_title: &str) -> Box<dyn Element> {
        let menu_btn = Container::new(
            ConstrainedBox::new(
                Align::new(icons::agent_icon("agent-more.svg", theme::muted())).finish(),
            )
            .with_width(28.0)
            .with_height(28.0)
            .finish(),
        )
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(6.0),
        ))
        .finish();

        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Shrinkable::new(
                        1.0,
                        ui_text::section_title(thread_title.to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .with_child(menu_btn)
                .finish(),
        )
        .with_padding_left(20.0)
        .with_padding_right(20.0)
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
        .with_background(agent_header_bg())
        .finish()
    }
}

impl Entity for AgentPanelView {
    type Event = ();
}

impl View for AgentPanelView {
    fn ui_name() -> &'static str {
        "AgentPanelView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let _ = self.generation.lock().map(|g| *g).unwrap_or(0);
        let state = self.state.lock().expect("agent panel state");
        let draft = state.draft.clone();
        let marked = state.field_state.marked_text.clone();
        let busy = state.busy;
        let _mode = state.mode;
        let input_focused = state.input_focused;
        let caret_blink = self.caret_blink.visible;
        let lines = Arc::clone(&state.lines);
        let demo_user_prompt = state.demo_user_prompt.clone();
        let demo_status_line = state.demo_status_line.clone();
        let demo_assistant_body = state.demo_assistant_body.clone();
        let demo_thinking = state.demo_thinking;
        let access_menu_open = state.access_menu_open;
        let model_menu_open = state.model_menu_open;
        let access_mode = self.access_mode;
        let projects = state.projects.clone();
        let sidebar_sessions = state.sidebar_sessions.clone();
        let active_project_id = state.active_project_id.clone();
        let active_sidebar_session_id = state.active_sidebar_session_id.clone();
        let thread_title = state.thread_title.clone();
        let sidebar_search = state.sidebar_search.clone();
        let sidebar_search_focused = state.sidebar_search_focused;
        drop(state);

        let preferred_agent = self.preferred_agent;

        let input_border = if input_focused {
            theme::accent_cool()
        } else {
            theme::border_bright()
        };

        let placeholder = if busy {
            "执行中…"
        } else {
            "输入后续修改或追问…"
        };

        let input_height = multiline_input::box_height(&draft, multiline_input::DEFAULT_COLS);

        let transcript_model = TranscriptViewModel {
            user_prompt: demo_user_prompt,
            status_line: demo_status_line,
            assistant_body: demo_assistant_body,
            thinking: demo_thinking,
            lines: lines.to_vec(),
        };

        let composer_body = Container::new(
            ConstrainedBox::new(
                Container::new(
                    Flex::column()
                        .with_child(
                            ConstrainedBox::new(
                                Container::new(
                                    EventHandler::new(self.agent_composer_input(
                                        &draft,
                                        &marked,
                                        input_focused,
                                        caret_blink,
                                        busy,
                                        placeholder,
                                    ))
                                    .on_left_mouse_down(|ctx, _, _| {
                                        ctx.dispatch_typed_action(AgentPanelAction::FocusInput);
                                        DispatchEventResult::StopPropagation
                                    })
                                    .finish(),
                                )
                                .with_padding_left(16.0)
                                .with_padding_right(16.0)
                                .with_padding_top(14.0)
                                .with_padding_bottom(8.0)
                                .finish(),
                            )
                            .with_height(input_height)
                            .with_max_height(multiline_input::box_height(
                                &"x".repeat(
                                    multiline_input::DEFAULT_COLS * multiline_input::MAX_LINES,
                                ),
                                multiline_input::DEFAULT_COLS,
                            ))
                            .finish(),
                        )
                        .with_child(self.composer_bar(busy, access_mode))
                        .finish(),
                )
                .with_background(theme::panel())
                .with_border(Border::all(1.0).with_border_color(input_border))
                .with_corner_radius(warpui::elements::CornerRadius::with_all(
                    warpui::elements::Radius::Pixels(16.0),
                ))
                .finish(),
            )
            .with_max_width(AGENT_THREAD_MAX_WIDTH)
            .finish(),
        )
        .finish();

        let composer = self.wrap_composer_with_popovers(
            composer_body,
            access_mode,
            access_menu_open,
            model_menu_open,
        );

        let thread_scroll = Container::new(
            Align::new(render_transcript(&transcript_model, self.font, self.mono))
                .finish(),
        )
        .with_padding_left(24.0)
        .with_padding_right(24.0)
        .with_padding_top(28.0)
        .with_padding_bottom(AGENT_THREAD_BOTTOM_PAD)
        .with_background(theme::canvas())
        .finish();

        let mut main_stack = Stack::new();
        main_stack.add_child(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(self.agent_header(&thread_title))
                .with_child(Expanded::new(1.0, thread_scroll).finish())
                .finish(),
        );
        main_stack.add_child(
            Align::new(
                Container::new(composer)
                    .with_padding_left(24.0)
                    .with_padding_right(24.0)
                    .with_padding_bottom(20.0)
                    .finish(),
            )
            .bottom_center()
            .finish(),
        );

        if access_menu_open || model_menu_open {
            main_stack.add_child(self.composer_menu_scrim());
        }

        let main_area = main_stack.finish();

        let shell = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                ConstrainedBox::new(sidebar::render_sidebar(
                    self.font,
                    self.sidebar_scroll.clone(),
                    &projects,
                    &sidebar_sessions,
                    &active_project_id,
                    &active_sidebar_session_id,
                    &sidebar_search,
                    sidebar_search_focused,
                ))
                .with_width(sidebar::SIDEBAR_WIDTH)
                .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(main_area)
                        .with_background(theme::canvas())
                        .finish(),
                )
                .finish(),
            );

        let panel = Container::new(shell.finish())
            .with_background(theme::canvas())
            .finish();

        tab_content_fill(
            EventHandler::new(panel)
                .with_always_handle()
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(AgentPanelAction::DismissComposerMenus);
                    DispatchEventResult::PropagateToParent
                })
                .on_keydown({
                    let state = Arc::clone(&self.state);
                    let notify_tx = self.generation_notify_tx.clone();
                    move |ctx, _, keystroke| {
                        if let Some(action) =
                            Self::keystroke_action(&state, preferred_agent, keystroke)
                        {
                            ctx.dispatch_typed_action(action);
                            return DispatchEventResult::StopPropagation;
                        }
                        let was_focused = state
                            .lock()
                            .map(|panel| panel.input_focused)
                            .unwrap_or(false);
                        if Self::handle_keystroke_panel(&state, keystroke, &notify_tx) {
                            let now_focused = state
                                .lock()
                                .map(|panel| panel.input_focused)
                                .unwrap_or(false);
                            if now_focused && !was_focused {
                                ctx.dispatch_typed_action(AgentPanelAction::FocusInput);
                            }
                            DispatchEventResult::StopPropagation
                        } else {
                            DispatchEventResult::PropagateToParent
                        }
                    }
                })
                .finish(),
        )
    }

    fn accessibility_contents(&self, _app: &AppContext) -> Option<AccessibilityContent> {
        let agent = match self.preferred_agent {
            PreferredAgent::Codex => "Codex",
            PreferredAgent::Cursor => "Cursor",
        };
        let mode = self
            .state
            .lock()
            .map(|p| {
                if p.mode == InteractionMode::Chat {
                    "对话"
                } else {
                    "全自动"
                }
            })
            .unwrap_or("对话");
        Some(AccessibilityContent::new(
            format!("Warp Agent，{agent}，{mode} 模式"),
            "左右方向键切换 Agent。上下方向键切换对话或全自动模式。Tab 聚焦输入框，Enter 发送。",
            WarpA11yRole::WindowRole,
        ))
    }

    fn accessibility_data(&self, _ctx: &mut ViewContext<Self>) -> Option<AccessibilityData> {
        Some(AccessibilityData {
            content: "Warp Agent 面板".into(),
        })
    }
}

impl TypedActionView for AgentPanelView {
    type Action = AgentPanelAction;

    fn handle_action(&mut self, action: &AgentPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            AgentPanelAction::SetVisible(visible) => self.set_tab_visible(*visible, ctx),
            AgentPanelAction::SelectAgent(agent) => self.select_agent(*agent, ctx),
            AgentPanelAction::SelectAccessMode(mode) => self.select_access_mode(*mode, ctx),
            AgentPanelAction::ToggleAccessMenu => self.toggle_access_menu(ctx),
            AgentPanelAction::ToggleModelMenu => self.toggle_model_menu(ctx),
            AgentPanelAction::DismissComposerMenus => self.dismiss_composer_menus(ctx),
            AgentPanelAction::SelectMode(mode) => self.select_mode(*mode, ctx),
            AgentPanelAction::Send => self.send_message(ctx),
            AgentPanelAction::PasteInput => self.paste_input(ctx),
            AgentPanelAction::FocusInput => {
                if let Ok(mut panel) = self.state.lock() {
                    if !panel.busy {
                        panel.input_focused = true;
                        panel.sidebar_search_focused = false;
                    }
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            AgentPanelAction::SelectProject(id) => self.select_project(id.clone(), ctx),
            AgentPanelAction::SelectSession(id) => self.select_session(id.clone(), ctx),
            AgentPanelAction::NewProject => self.new_project(ctx),
            AgentPanelAction::NewThread => self.new_thread(ctx),
            AgentPanelAction::FocusSidebarSearch => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.sidebar_search_focused = true;
                    panel.input_focused = false;
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            AgentPanelAction::SidebarMore => {}
            AgentPanelAction::NewConversation => self.new_conversation(ctx),
            AgentPanelAction::LaunchTerminal => self.launch_terminal(ctx),
            AgentPanelAction::RefreshStatus => self.refresh_status(ctx),
            AgentPanelAction::Stop => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.busy = false;
                    panel.demo_thinking = false;
                    panel.session_running = false;
                    if panel.demo_status_line.is_some() {
                        panel.demo_status_line = Some("已完成".into());
                    }
                }
                ctx.notify();
            }
            AgentPanelAction::TextFieldEdit(edit) => {
                if let Ok(mut panel) = self.state.lock() {
                    if !panel.busy {
                        let edit = edit.clone();
                        let mut draft = std::mem::take(&mut panel.draft);
                        panel.field_state.apply(&mut draft, &edit);
                        panel.draft = draft;
                        panel.input_focused = true;
                    }
                }
                sync_caret_blink(self, ctx);
                self.bump();
                ctx.notify();
            }
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &AgentPanelAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            AgentPanelAction::SelectAgent(PreferredAgent::Codex) => {
                AccessibilityContent::new_without_help("选择 GPT-5.5", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::SelectAgent(PreferredAgent::Cursor) => {
                AccessibilityContent::new_without_help(
                    "选择 Cursor Agent",
                    WarpA11yRole::ButtonRole,
                )
            }
            AgentPanelAction::SelectAccessMode(AgentAccessMode::FullAccess) => {
                AccessibilityContent::new_without_help("完全访问", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::SelectAccessMode(AgentAccessMode::WorkspaceWrite) => {
                AccessibilityContent::new_without_help("工作区写入", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::ToggleAccessMenu => {
                AccessibilityContent::new_without_help("访问权限菜单", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::ToggleModelMenu => {
                AccessibilityContent::new_without_help("模型菜单", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::DismissComposerMenus => {
                return ActionAccessibilityContent::Empty;
            }
            AgentPanelAction::SelectMode(InteractionMode::Chat) => {
                AccessibilityContent::new_without_help("对话模式", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::SelectMode(InteractionMode::Task) => {
                AccessibilityContent::new_without_help("全自动模式", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::Send => {
                AccessibilityContent::new_without_help("发送消息", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::FocusInput => {
                AccessibilityContent::new_without_help("聚焦输入框", WarpA11yRole::TextfieldRole)
            }
            AgentPanelAction::NewConversation => {
                AccessibilityContent::new_without_help("新对话", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::LaunchTerminal => {
                AccessibilityContent::new_without_help("打开系统终端", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::PasteInput => {
                AccessibilityContent::new_without_help("粘贴到输入框", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::SelectProject(_) | AgentPanelAction::SelectSession(_) => {
                AccessibilityContent::new_without_help("选择侧栏项", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::NewProject | AgentPanelAction::NewThread => {
                AccessibilityContent::new_without_help("新建侧栏项", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::FocusSidebarSearch => {
                AccessibilityContent::new_without_help("聚焦会话搜索", WarpA11yRole::TextfieldRole)
            }
            AgentPanelAction::SidebarMore => {
                AccessibilityContent::new_without_help("侧栏菜单", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::Stop => {
                AccessibilityContent::new_without_help("停止 Agent", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::TextFieldEdit(_) => {
                AccessibilityContent::new_without_help("编辑 Agent 输入", WarpA11yRole::TextfieldRole)
            }
            AgentPanelAction::SetVisible(_) | AgentPanelAction::RefreshStatus => {
                return ActionAccessibilityContent::Empty;
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
}

impl CaretBlinkHost for AgentPanelView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.state
            .lock()
            .map(|panel| panel.input_focused && !panel.busy)
            .unwrap_or(false)
    }
}
