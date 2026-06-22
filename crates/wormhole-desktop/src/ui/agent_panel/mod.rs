mod transcript;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use transcript::{render_transcript, TranscriptLine};
use warpui::elements::{
    Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
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
    lines: Vec<TranscriptLine>,
    chat_messages: Vec<AgentLlmChatMessage>,
    resume_id: Option<String>,
    active_session_id: Option<String>,
    event_cursor: u64,
    polling_session: bool,
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
    input_focused: bool,
}

impl AgentPanelView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let preferred_agent = warp_embed_prefs::load_prefs(&core.data_dir()).preferred_agent;
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
                lines: Vec::new(),
                chat_messages: Vec::new(),
                resume_id: None,
                active_session_id: None,
                event_cursor: 0,
                polling_session: false,
            })),
            generation: Arc::new(Mutex::new(0)),
            event_poll_inflight: Arc::new(AtomicBool::new(false)),
            visible: false,
            preferred_agent,
            input_focused: true,
        }
    }

    pub fn focus_with_agent(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        self.preferred_agent = agent;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = core.block_on(async {
                warp_embed_prefs::set_preferred_agent(&data_dir, agent).await
            });
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
            self.start_ui_poll(ctx);
        }
        ctx.notify();
    }

    fn bump(&self) {
        if let Ok(mut gen) = self.generation.lock() {
            *gen = gen.saturating_add(1);
        }
    }

    fn start_ui_poll(&self, ctx: &mut ViewContext<Self>) {
        let generation = Arc::clone(&self.generation);
        let last = Arc::new(Mutex::new(0u64));
        let state = Arc::clone(&self.state);
        let core = self.core.clone();
        let event_poll_inflight = Arc::clone(&self.event_poll_inflight);
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(350));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::ui_poll_once(
            ctx,
            tick_rx,
            generation,
            last,
            state,
            core,
            event_poll_inflight,
        );
    }

    fn ui_poll_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        generation: Arc<Mutex<u64>>,
        last: Arc<Mutex<u64>>,
        state: Arc<Mutex<PanelState>>,
        core: CoreHandle,
        event_poll_inflight: Arc<AtomicBool>,
    ) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_err() {
                    return;
                }
                let current = generation.lock().map(|g| *g).unwrap_or(0);
                let prev = last.lock().map(|g| *g).unwrap_or(0);
                if current != prev {
                    if let Ok(mut guard) = last.lock() {
                        *guard = current;
                    }
                    ctx.notify();
                }

                let should_poll = state
                    .lock()
                    .ok()
                    .map(|panel| panel.polling_session && panel.active_session_id.is_some())
                    .unwrap_or(false);
                if should_poll && !event_poll_inflight.swap(true, Ordering::SeqCst) {
                    let (session_id, cursor) = {
                        let panel = state.lock().expect("agent panel state");
                        (
                            panel.active_session_id.clone().expect("session id"),
                            panel.event_cursor,
                        )
                    };
                    let state_for_task = Arc::clone(&state);
                    let generation_for_task = Arc::clone(&generation);
                    let inflight = Arc::clone(&event_poll_inflight);
                    let core_for_task = core.clone();
                    ctx.spawn(
                        async move {
                            agent_read_local_session_events(
                                session_id,
                                cursor,
                                core_for_task.app_state(),
                            )
                            .await
                        },
                        move |_view, output, ctx| {
                            inflight.store(false, Ordering::SeqCst);
                            if let Ok(page) = output {
                                let mut panel = state_for_task.lock().expect("agent panel state");
                                for event in page.events {
                                    panel.lines.push(TranscriptLine {
                                        channel: event.channel,
                                        text: event.text,
                                        level: event.level,
                                    });
                                }
                                panel.event_cursor = page.next_cursor;
                                if matches!(page.status.as_str(), "completed" | "failed") {
                                    panel.polling_session = false;
                                    panel.busy = false;
                                    panel.active_session_id = None;
                                    panel.status = format!("任务{}", page.status);
                                }
                            }
                            if let Ok(mut gen) = generation_for_task.lock() {
                                *gen = gen.saturating_add(1);
                            }
                            ctx.notify();
                        },
                    );
                }

                if view.visible {
                    Self::ui_poll_once(
                        ctx,
                        tick_rx,
                        generation,
                        last,
                        state,
                        core,
                        event_poll_inflight,
                    );
                }
            },
        );
    }

    fn select_agent(&mut self, agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
        if self.preferred_agent == agent {
            return;
        }
        self.preferred_agent = agent;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = core.block_on(async {
                warp_embed_prefs::set_preferred_agent(&data_dir, agent).await
            });
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
            state.lines.clear();
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
            state.lines.push(TranscriptLine {
                channel: "user".into(),
                text: text.clone(),
                level: "info".into(),
            });
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
        let generation_async = Arc::clone(&self.generation);
        let generation_callback = Arc::clone(&self.generation);
        let agent = self.preferred_agent;
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
                        let stream_gen = Arc::clone(&generation_async);
                        agent_codex_chat_with_stream(params, state, move |event| {
                            let level = if event.channel == "error" {
                                "error".to_string()
                            } else {
                                "info".to_string()
                            };
                            if let Ok(mut panel) = stream_shared.lock() {
                                panel.lines.push(TranscriptLine {
                                    channel: event.channel,
                                    text: event.text,
                                    level,
                                });
                            }
                            if let Ok(mut gen) = stream_gen.lock() {
                                *gen = gen.saturating_add(1);
                            }
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
                        panel.lines.push(TranscriptLine {
                            channel: "assistant".into(),
                            text: reply.content.clone(),
                            level: "info".into(),
                        });
                        panel.chat_messages.push(AgentLlmChatMessage {
                            role: "assistant".into(),
                            content: reply.content,
                        });
                        panel.resume_id = reply.thread_id;
                        panel.status = format!("完成 · {}", reply.model);
                    }
                    Err(err) => {
                        panel.lines.push(TranscriptLine {
                            channel: "stderr".into(),
                            text: err.clone(),
                            level: "error".into(),
                        });
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
                let request = AgentStartRequest {
                    prompt,
                    cwd: None,
                };
                let state = core.app_state();
                match agent {
                    PreferredAgent::Codex => agent_start_session(request, state).await,
                    PreferredAgent::Cursor => agent_cursor_start_session(request, state).await,
                }
            },
            move |_view, output, ctx| {
                let mut panel = shared.lock().expect("agent panel state");
                match output {
                    Ok(session) => {
                        panel.lines.push(TranscriptLine {
                            channel: "status".into(),
                            text: format!("任务已启动 · session {}", session.id),
                            level: "info".into(),
                        });
                        panel.active_session_id = Some(session.id);
                        panel.event_cursor = 0;
                        panel.polling_session = true;
                        panel.status = "任务运行中…".into();
                    }
                    Err(err) => {
                        panel.lines.push(TranscriptLine {
                            channel: "stderr".into(),
                            text: err.clone(),
                            level: "error".into(),
                        });
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
            .with_uniform_padding(8.0)
            .with_background(bg)
            .finish();
            row.add_child(btn);
        }
        Container::new(row.finish())
            .with_uniform_padding(4.0)
            .with_background(theme::panel())
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
        let mode = self
            .state
            .lock()
            .expect("agent panel state")
            .mode;
        let chat = mode == InteractionMode::Chat;
        self.segmented(
            &[("对话", chat), ("全自动", !chat)],
            |idx| {
                AgentPanelAction::SelectMode(if idx == 0 {
                    InteractionMode::Chat
                } else {
                    InteractionMode::Task
                })
            },
        )
    }

    fn toolbar_button(&self, label: &str, action: AgentPanelAction) -> Box<dyn Element> {
        let label_el = ui_text::body(label.to_string(), self.font)
            .with_color(theme::accent())
            .finish();
        Container::new(
            EventHandler::new(label_el)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(24))
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
        let lines = state.lines.clone();
        drop(state);

        let input_border = if self.input_focused {
            theme::accent()
        } else {
            theme::border()
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
                    .with_child(self.toolbar_button("粘贴", AgentPanelAction::PasteInput))
                    .with_child(self.toolbar_button("发送", AgentPanelAction::Send))
                    .with_child(self.toolbar_button("新对话", AgentPanelAction::NewConversation))
                    .with_child(self.toolbar_button(
                        "系统终端",
                        AgentPanelAction::LaunchTerminal,
                    ))
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
                    Container::new(render_transcript(&lines, self.font, self.mono))
                        .with_uniform_padding(8.0)
                        .with_background(ColorU::new(248, 248, 250, 255))
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Container::new(
                    EventHandler::new(
                        ui_text::mono(
                            if draft.is_empty() {
                                if busy {
                                    "执行中…".to_string()
                                } else {
                                    "从剪贴板粘贴后点击「发送」".to_string()
                                }
                            } else {
                                draft
                            },
                            self.mono,
                        )
                        .with_color(if busy {
                            theme::muted()
                        } else {
                            theme::text()
                        })
                        .finish(),
                    )
                    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                    .finish(),
                )
                .with_uniform_padding(10.0)
                .with_background(theme::panel())
                .with_border(warpui::elements::Border::all(1.0).with_border_color(input_border))
                .finish(),
            );

        Container::new(body.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
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
            AgentPanelAction::NewConversation => self.new_conversation(ctx),
            AgentPanelAction::LaunchTerminal => self.launch_terminal(ctx),
            AgentPanelAction::RefreshStatus => self.refresh_status(ctx),
        }
    }
}
