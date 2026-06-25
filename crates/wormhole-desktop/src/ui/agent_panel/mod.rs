mod transcript;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use transcript::{render_transcript, TranscriptLine};
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Border, Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{
    AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext,
};
use warpui_core::keymap::Keystroke;
use wormhole_desktop_core::agent_llm_commands::{AgentLlmChatMessage, AgentLlmChatParams};
use wormhole_desktop_core::warp_embed_prefs::{self, PreferredAgent};
use wormhole_desktop_core::{
    agent_codex_chat_with_stream, agent_cursor_chat, agent_cursor_start_session,
    agent_launch_codex_terminal, agent_launch_cursor_terminal, agent_read_local_session_events,
    agent_start_session, agent_status, AgentStartRequest,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Chat,
    Task,
}

#[derive(Debug, Clone)]
pub enum AgentPanelAction {
    SetVisible(bool),
    SelectAgent(PreferredAgent),
    SelectMode(InteractionMode),
    Send,
    PasteInput,
    FocusInput,
    NewConversation,
    LaunchTerminal,
    RefreshStatus,
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
    generation_notify_tx: async_channel::Sender<()>,
    generation_notify_rx: async_channel::Receiver<()>,
}

impl AgentPanelView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let preferred_agent = warp_embed_prefs::load_prefs(&core.data_dir()).preferred_agent;
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
            })),
            generation: Arc::new(Mutex::new(0)),
            event_poll_inflight: Arc::new(AtomicBool::new(false)),
            visible: false,
            preferred_agent,
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
            self.refresh_status(ctx);
            self.start_generation_listener(ctx);
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
                    agent_read_local_session_events(
                        session_id,
                        cursor,
                        core_for_async.app_state(),
                    )
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
                panel.input_focused = !panel.input_focused;
            }
            let _ = notify_tx.try_send(());
            return true;
        }
        let mut panel = state.lock().expect("agent panel state");
        if !panel.input_focused || panel.busy {
            return false;
        }
        match keystroke.key.as_str() {
            "enter" | "return" => {
                if !panel.draft.trim().is_empty() {
                    panel.pending_send = true;
                }
            }
            "backspace" => {
                panel.draft.pop();
            }
            "escape" => {
                panel.draft.clear();
                panel.input_focused = false;
            }
            key if key.len() == 1 => {
                if let Some(ch) = key.chars().next() {
                    if !keystroke.ctrl && !keystroke.meta {
                        panel.draft.push(ch);
                    }
                }
            }
            _ => return false,
        }
        drop(panel);
        let _ = notify_tx.try_send(());
        true
    }

    fn select_agent(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        if self.preferred_agent == agent {
            return;
        }
        self.preferred_agent = agent;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = core
                .block_on(async { warp_embed_prefs::set_preferred_agent(&data_dir, agent).await });
        });
        self.update_status_line(ctx);
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
        let status = state.status.clone();
        let busy = state.busy;
        let input_focused = state.input_focused;
        let lines = Arc::clone(&state.lines);
        drop(state);

        let preferred_agent = self.preferred_agent;

        let input_border = if input_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };

        let placeholder = if busy {
            "执行中…"
        } else if input_focused {
            "输入消息，Enter 发送，Esc 取消焦点"
        } else {
            "点击输入框或按 Tab 聚焦，Enter 发送"
        };

        let draft_empty = draft.is_empty();
        let input_text = if draft_empty {
            placeholder.to_string()
        } else {
            draft
        };

        let body = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::body("Warp Agent", self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(self.agent_switcher())
            .with_child(self.mode_switcher())
            .with_child(
                Flex::row()
                    .with_child(self.toolbar_button("粘贴", AgentPanelAction::PasteInput, false))
                    .with_child(self.toolbar_button("发送", AgentPanelAction::Send, true))
                    .with_child(
                        self.toolbar_button("新对话", AgentPanelAction::NewConversation, false),
                    )
                    .with_child(
                        self.toolbar_button("系统终端", AgentPanelAction::LaunchTerminal, false),
                    )
                    .finish(),
            )
            .with_child(
                ui_text::body(status, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.0,
                    Container::new(render_transcript(&lines, self.font, self.mono))
                        .with_uniform_padding(8.0)
                        .with_background(theme::bg())
                        .with_border(Border::all(1.0).with_border_color(theme::border()))
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Container::new(
                    EventHandler::new(
                        ui_text::mono(input_text, self.mono)
                        .with_color(if busy {
                            theme::placeholder()
                        } else if draft_empty {
                            theme::placeholder()
                        } else {
                            theme::text()
                        })
                        .finish(),
                    )
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(AgentPanelAction::FocusInput);
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_uniform_padding(12.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_color(input_border))
                .finish(),
            );

        let panel = Container::new(body.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish();

        EventHandler::new(panel)
            .with_always_handle()
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
                    if Self::handle_keystroke_panel(&state, keystroke, &notify_tx) {
                        DispatchEventResult::StopPropagation
                    } else {
                        DispatchEventResult::PropagateToParent
                    }
                }
            })
            .finish()
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
            AgentPanelAction::SelectMode(mode) => self.select_mode(*mode, ctx),
            AgentPanelAction::Send => self.send_message(ctx),
            AgentPanelAction::PasteInput => self.paste_input(ctx),
            AgentPanelAction::FocusInput => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.input_focused = true;
                }
                ctx.notify();
            }
            AgentPanelAction::NewConversation => self.new_conversation(ctx),
            AgentPanelAction::LaunchTerminal => self.launch_terminal(ctx),
            AgentPanelAction::RefreshStatus => self.refresh_status(ctx),
        }
    }

    fn action_accessibility_contents(
        &mut self,
        action: &AgentPanelAction,
        _ctx: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        let content = match action {
            AgentPanelAction::SelectAgent(PreferredAgent::Codex) => AccessibilityContent::new_without_help(
                "选择 Codex Agent",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::SelectAgent(PreferredAgent::Cursor) => AccessibilityContent::new_without_help(
                "选择 Cursor Agent",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::SelectMode(InteractionMode::Chat) => AccessibilityContent::new_without_help(
                "对话模式",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::SelectMode(InteractionMode::Task) => AccessibilityContent::new_without_help(
                "全自动模式",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::Send => {
                AccessibilityContent::new_without_help("发送消息", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::FocusInput => AccessibilityContent::new_without_help(
                "聚焦输入框",
                WarpA11yRole::TextfieldRole,
            ),
            AgentPanelAction::NewConversation => AccessibilityContent::new_without_help(
                "新对话",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::LaunchTerminal => AccessibilityContent::new_without_help(
                "打开系统终端",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::PasteInput => AccessibilityContent::new_without_help(
                "粘贴到输入框",
                WarpA11yRole::ButtonRole,
            ),
            AgentPanelAction::SetVisible(_) | AgentPanelAction::RefreshStatus => {
                return ActionAccessibilityContent::Empty;
            }
        };
        ActionAccessibilityContent::Custom(content)
    }
}
