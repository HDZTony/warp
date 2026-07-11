mod composer_add;
mod composer_menus;
mod project_create_modal;
mod project_delete_modal;
pub mod sidebar;
mod transcript;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use composer_add::{
    ComposerAttachment, FilesModalState, MediaModalState, ADD_POPOVER_INSET_LEFT, DEMO_FILES,
    DEMO_MEDIA,
};
use composer_menus::{access_label, composer_model_chip_label};
use project_create_modal::{ProjectCreateState, ProjectCreateStep};
use project_delete_modal::ProjectDeleteState;
use wormhole_desktop_core::state::resolve_agent_workspace_cwd;
use pathfinder_color::ColorU;
use transcript::{render_transcript, TranscriptLine, TranscriptViewModel};
use warpui::accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole};
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CrossAxisAlignment, DispatchEventResult, Empty, EventHandler, Expanded, Fill, Flex,
    MainAxisSize, ParentElement, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::{AccessibilityData, AppContext, Element, Entity, TypedActionView, View, ViewContext};
use warpui_core::keymap::Keystroke;
use wormhole_desktop_core::agent_llm_commands::{AgentLlmChatMessage, AgentLlmChatParams};
use wormhole_desktop_core::warp_embed_prefs::{self, AgentAccessMode, AgentModelRate, PreferredAgent};
use wormhole_desktop_core::{
    agent_chat_with_codex_fallback, agent_launch_terminal_with_codex_fallback,
    agent_list_sessions, agent_read_local_session_events,
    agent_start_session_with_codex_fallback, agent_status, AgentSessionDto, AgentStartRequest,
    AgentTurnBackend,
};

use crate::ui::clipboard::write_clipboard_text;
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::multiline_input;
use crate::ui::panel_primitives::{
    agent_header_bg, center_composer_width, tab_content_fill, AGENT_THREAD_BOTTOM_PAD,
    AGENT_THREAD_MAX_WIDTH,
};
use crate::ui::text_field_input::{
    render_field_with_caret, sync_caret_blink, CaretBlink, CaretBlinkHost, TextFieldEditAction,
    TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

const COMPOSER_BAR_LIFT: f32 = 44.0;
const ACCESS_POPOVER_INSET_LEFT: f32 = 46.0;
/// composer_bar: padding_top 6 + padding_bottom 10 + 32px controls.
const COMPOSER_CHROME_HEIGHT: f32 = 48.0;
/// `#agent-composer-folder` min-height (HTML) + wrap `gap: 8px`.
const COMPOSER_FOLDER_BAR_HEIGHT: f32 = 40.0;
const COMPOSER_FOLDER_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    Chat,
    Task,
}

#[derive(Debug, Clone)]
pub enum AgentPanelAction {
    SetVisible(bool),
    SelectAccessMode(AgentAccessMode),
    ToggleAccessMenu,
    SelectModelRate(AgentModelRate),
    ToggleModelMenu,
    ToggleAddMenu,
    OpenFilesModal,
    CloseFilesModal,
    SelectDemoFile(usize),
    ConfirmFilesModal,
    OpenMediaModal,
    CloseMediaModal,
    SelectDemoMedia(usize),
    ConfirmMediaModal,
    ToggleGoalMode,
    TogglePlanMode,
    ClearGoalMode,
    ClearPlanMode,
    RemoveAttachment(usize),
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
    NewStandaloneChat,
    NewProjectThread(String),
    ToggleProjectExpanded(String),
    OpenProjectCreateModal,
    CloseProjectCreateModal,
    ProjectCreateNext,
    ProjectCreateUseExistingFolder,
    ProjectCreateSubmit,
    FocusProjectCreateName,
    ProjectCreateNameEdit(TextFieldEditAction),
    OpenProjectRowMenu(String),
    OpenProjectDeleteModal(String),
    CloseProjectDeleteModal,
    SetProjectDeleteMode(bool),
    ConfirmDeleteProject,
    RenameProject(String),
    DeleteActiveStandaloneChat,
    FocusSidebarSearch,
    Stop,
    ToggleChatsMenu,
    DismissSidebarMenus,
    DeleteActiveProject,
    ArchiveSession(String),
    ArchiveActiveSession,
    RestoreSession(String),
    DeleteSession(String),
    OpenSessionContextMenu {
        id: String,
        archived: bool,
        x: f32,
        y: f32,
    },
    CloseSessionContextMenu,
    CopyUserPrompt,
    ClearAndUnfocus,
    ToggleComposerFocus,
    TextFieldEdit(TextFieldEditAction),
    SetSidebarHover(Option<String>),
    ClearSidebarHoverIf(String),
    SetProjectHeadHover(Option<String>),
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
    archived_ids: HashSet<String>,
    expanded_project_ids: HashSet<String>,
    project_create: Option<ProjectCreateState>,
    /// Project-row ⋯ menu target id — HTML `#agent-projects-menu` (in-tree anchor).
    project_row_menu: Option<String>,
    /// Delete confirmation — HTML `#project-delete-modal`.
    project_delete: Option<ProjectDeleteState>,
    chats_menu_open: bool,
    session_context_menu: Option<(String, bool, f32, f32)>,
    access_menu_open: bool,
    model_menu_open: bool,
    add_menu_open: bool,
    plan_mode: bool,
    goal_mode: bool,
    attachments: Vec<ComposerAttachment>,
    files_modal: Option<FilesModalState>,
    media_modal: Option<MediaModalState>,
    field_state: TextFieldState,
    sidebar_hover: Option<String>,
    project_head_hover: Option<String>,
}

pub struct AgentPanelView {
    core: CoreHandle,
    font: FamilyId,
    mono: FamilyId,
    state: Arc<Mutex<PanelState>>,
    generation: Arc<Mutex<u64>>,
    event_poll_inflight: Arc<AtomicBool>,
    visible: bool,
    access_mode: AgentAccessMode,
    model_rate: AgentModelRate,
    sidebar_scroll: ClippedScrollStateHandle,
    thread_scroll: ClippedScrollStateHandle,
    caret_blink: CaretBlink,
    generation_notify_tx: async_channel::Sender<()>,
    generation_notify_rx: async_channel::Receiver<()>,
}

impl AgentPanelView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let prefs = warp_embed_prefs::load_prefs(&core.data_dir());
        let access_mode = prefs.access_mode;
        let model_rate = prefs.model_rate;
        let (generation_notify_tx, generation_notify_rx) = async_channel::unbounded();
        let data_dir = core.data_dir();
        let archived_ids = sidebar::load_archived_ids(&data_dir);
        let (projects, expanded_project_ids) = sidebar::load_projects_state(&data_dir);
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
                projects,
                sidebar_sessions: Vec::new(),
                active_project_id: String::new(),
                active_sidebar_session_id: String::new(),
                thread_title: "新会话".into(),
                sidebar_search: String::new(),
                sidebar_search_focused: false,
                archived_ids,
                expanded_project_ids,
                project_create: None,
                project_row_menu: None,
                project_delete: None,
                chats_menu_open: false,
                session_context_menu: None,
                access_menu_open: false,
                model_menu_open: false,
                add_menu_open: false,
                plan_mode: false,
                goal_mode: false,
                attachments: Vec::new(),
                files_modal: None,
                media_modal: None,
                field_state: TextFieldState::new(),
                sidebar_hover: None,
                project_head_hover: None,
            })),
            generation: Arc::new(Mutex::new(0)),
            event_poll_inflight: Arc::new(AtomicBool::new(false)),
            visible: false,
            access_mode,
            model_rate,
            sidebar_scroll: ClippedScrollStateHandle::default(),
            thread_scroll: ClippedScrollStateHandle::default(),
            caret_blink: CaretBlink::new(),
            generation_notify_tx,
            generation_notify_rx,
        }
    }

    fn unix_now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn format_relative_time(started_at: u64) -> String {
        let now = Self::unix_now_secs();
        let secs = now.saturating_sub(started_at);
        if secs < 60 {
            "now".into()
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else if secs < 86_400 {
            format!("{}h", secs / 3600)
        } else {
            format!("{}d", secs / 86_400)
        }
    }

    fn session_label_from_dto(dto: &AgentSessionDto) -> String {
        dto.prompt
            .as_deref()
            .map(|p| p.lines().next().unwrap_or(p).trim())
            .filter(|line| !line.is_empty())
            .map(|line| {
                if line.chars().count() > 32 {
                    format!("{}…", line.chars().take(31).collect::<String>())
                } else {
                    line.to_string()
                }
            })
            .unwrap_or_else(|| dto.id.chars().take(12).collect())
    }

    fn is_local_sidebar_session(session_id: &str) -> bool {
        session_id.starts_with("session-")
    }

    fn composer_send_on_enter(busy: bool, shift: bool, draft: &str) -> bool {
        !busy && !shift && !draft.trim().is_empty()
    }

    fn map_session_dto(dto: AgentSessionDto, project_id: Option<String>) -> sidebar::AgentSession {
        let label = Self::session_label_from_dto(&dto);
        sidebar::AgentSession {
            id: dto.id,
            label,
            project_id,
            time: Self::format_relative_time(dto.started_at),
            running: dto.status == "running",
            prompt: dto.prompt.unwrap_or_default(),
        }
    }

    fn persist_projects(panel: &PanelState, data_dir: &std::path::Path) {
        sidebar::save_projects_state(data_dir, &panel.projects, &panel.expanded_project_ids);
    }

    fn pick_project_folder(title: &str) -> Option<PathBuf> {
        #[cfg(windows)]
        {
            return wormhole_desktop_platform_windows::pick_folder(title);
        }
        #[cfg(not(windows))]
        {
            return rfd::FileDialog::new().set_title(title).pick_folder();
        }
    }

    async fn resolve_session_cwd(
        panel: &PanelState,
        data_dir: &std::path::Path,
    ) -> Result<Option<String>, String> {
        if panel.active_sidebar_session_id.is_empty() {
            return Ok(None);
        }
        let session = panel
            .sidebar_sessions
            .iter()
            .find(|s| s.id == panel.active_sidebar_session_id);
        let Some(session) = session else {
            return Ok(None);
        };
        match &session.project_id {
            Some(pid) => {
                let project = panel.projects.iter().find(|p| p.id == *pid);
                let Some(project) = project else {
                    return Err(format!("未找到项目 {pid}"));
                };
                if !project.folder_path.is_dir() {
                    return Err(format!(
                        "项目目录不存在：{}",
                        project.folder_path.display()
                    ));
                }
                Ok(Some(project.folder_path.display().to_string()))
            }
            None => {
                let cwd = resolve_agent_workspace_cwd(data_dir).await;
                Ok(Some(cwd.display().to_string()))
            }
        }
    }

    fn is_idle_composer_state(active_sidebar_session_id: &str) -> bool {
        active_sidebar_session_id.is_empty()
    }

    fn insert_new_session(panel: &mut PanelState, project_id: Option<String>) -> String {
        let session_id = format!("session-{}", Self::unix_now_secs());
        let session = sidebar::AgentSession {
            id: session_id.clone(),
            label: "新会话".into(),
            project_id,
            time: "刚刚".into(),
            running: false,
            prompt: "描述你想让 AI 执行的任务…".into(),
        };
        panel.sidebar_sessions.insert(0, session);
        panel.active_sidebar_session_id = session_id.clone();
        panel.thread_title = "新会话".into();
        panel.lines = Arc::new(Vec::new());
        panel.chat_messages.clear();
        panel.resume_id = None;
        panel.active_session_id = None;
        panel.event_cursor = 0;
        panel.polling_session = false;
        panel.busy = false;
        panel.draft.clear();
        panel.sidebar_search.clear();
        panel.input_focused = true;
        panel.sidebar_search_focused = false;
        session_id
    }

    fn ensure_sidebar_session_for_send(panel: &mut PanelState) {
        if !Self::is_idle_composer_state(&panel.active_sidebar_session_id) {
            return;
        }
        let project_id = if panel.active_project_id.is_empty() {
            None
        } else {
            Some(panel.active_project_id.clone())
        };
        let draft = panel.draft.clone();
        Self::insert_new_session(panel, project_id);
        panel.draft = draft;
    }

    fn upsert_sidebar_session(panel: &mut PanelState, session: sidebar::AgentSession) {
        if let Some(existing) = panel
            .sidebar_sessions
            .iter_mut()
            .find(|s| s.id == session.id)
        {
            *existing = session;
        } else {
            panel.sidebar_sessions.insert(0, session);
        }
    }

    fn reload_sidebar_sessions(&self, ctx: &mut ViewContext<Self>) {
        let shared = Arc::clone(&self.state);
        let core = self.core.clone();
        ctx.spawn(
            async move { agent_list_sessions(core.app_state()).await },
            move |_view, output, ctx| {
                if let Ok(dtos) = output {
                    let mut panel = shared.lock().expect("agent panel state");
                    let project_id = panel.active_project_id.clone();
                    for dto in dtos {
                        let id = dto.id.clone();
                        if let Some(existing) =
                            panel.sidebar_sessions.iter_mut().find(|s| s.id == id)
                        {
                            existing.running = dto.status == "running";
                            existing.time = Self::format_relative_time(dto.started_at);
                            existing.label = Self::session_label_from_dto(&dto);
                            if let Some(prompt) = dto.prompt {
                                existing.prompt = prompt;
                            }
                        } else if !project_id.is_empty() {
                            panel.sidebar_sessions.push(Self::map_session_dto(
                                dto,
                                Some(project_id.clone()),
                            ));
                        }
                    }
                    drop(panel);
                    ctx.notify();
                }
            },
        );
    }

    fn load_session_transcript(
        &mut self,
        session_id: String,
        running: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let shared = Arc::clone(&self.state);
        let core = self.core.clone();
        let session_id_for_poll = session_id.clone();
        ctx.spawn(
            async move { agent_read_local_session_events(session_id, 0, core.app_state()).await },
            move |view, output, ctx| {
                match output {
                    Ok(page) => {
                        let mut panel = shared.lock().expect("agent panel state");
                        panel.lines = Arc::new(Vec::new());
                        panel.event_cursor = page.next_cursor;
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
                        panel.active_session_id = Some(session_id_for_poll.clone());
                        if running && page.status == "running" {
                            panel.polling_session = true;
                            panel.busy = true;
                        } else {
                            panel.polling_session = false;
                            panel.busy = false;
                        }
                        drop(panel);
                        view.bump();
                        if running && page.status == "running" {
                            view.start_session_poll(ctx);
                        }
                    }
                    Err(_) => {
                        let mut panel = shared.lock().expect("agent panel state");
                        panel.polling_session = false;
                        panel.busy = false;
                    }
                }
                ctx.notify();
            },
        );
    }

    pub fn focus_with_agent(&mut self, _agent: PreferredAgent, ctx: &mut ViewContext<Self>) {
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
                            panel.status = format!("任务{}", page.status);
                            if !panel.active_sidebar_session_id.is_empty() {
                                let id = panel.active_sidebar_session_id.clone();
                                if let Some(session) =
                                    panel.sidebar_sessions.iter_mut().find(|s| s.id == id)
                                {
                                    session.running = false;
                                }
                            }
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

    fn append_chat_completion_transcript(
        panel: &mut PanelState,
        backend: AgentTurnBackend,
        content: &str,
    ) {
        if backend == AgentTurnBackend::Codex {
            return;
        }
        Self::push_line(
            panel,
            TranscriptLine {
                channel: "assistant".into(),
                text: content.to_string(),
                level: "info".into(),
            },
        );
    }

    fn push_cursor_fallback_status(panel: &mut PanelState, codex_error: &str) {
        Self::push_line(
            panel,
            TranscriptLine {
                channel: "status".into(),
                text: format!("Codex 不可用，已改用 Cursor（后备）: {codex_error}"),
                level: "warning".into(),
            },
        );
    }

    fn keystroke_action(
        state: &Arc<Mutex<PanelState>>,
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
        if keystroke.key == "escape" {
            if let Ok(mut panel) = state.lock() {
                if panel.project_create.is_some() {
                    panel.project_create = None;
                    let _ = notify_tx.try_send(());
                    return true;
                }
            }
        }
        if keystroke.key == "tab" {
            if let Ok(mut panel) = state.lock() {
                if panel.sidebar_search_focused {
                    panel.sidebar_search_focused = false;
                    panel.input_focused = true;
                } else {
                    panel.input_focused = !panel.input_focused;
                    panel.sidebar_search_focused = false;
                }
            }
            let _ = notify_tx.try_send(());
            return true;
        }
        let mut panel = state.lock().expect("agent panel state");
        if let Some(create) = panel.project_create.as_mut() {
            let focused_name = create.name_focused;
            match keystroke.key.as_str() {
                "tab" => {}
                "enter" | "return" => {
                    drop(panel);
                    let _ = notify_tx.try_send(());
                    return false;
                }
                "backspace" => {
                    if focused_name {
                        create.name.pop();
                    }
                }
                key if key.len() == 1 => {
                    if !keystroke.ctrl && !keystroke.meta {
                        if let Some(ch) = key.chars().next() {
                            if focused_name {
                                create.name.push(ch);
                                create.name_invalid = false;
                            }
                        }
                    }
                }
                _ => return false,
            }
            drop(panel);
            let _ = notify_tx.try_send(());
            return true;
        }
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
            panel.active_project_id = project_id.clone();
            panel.expanded_project_ids.insert(project_id.clone());
            panel
                .sidebar_sessions
                .iter()
                .find(|s| {
                    s.project_id.as_deref() == Some(project_id.as_str())
                        && !panel.archived_ids.contains(&s.id)
                })
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
        let Some(session) = snapshot else {
            return;
        };
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.active_sidebar_session_id = session_id.clone();
            panel.thread_title = session.label.clone();
            if let Some(pid) = &session.project_id {
                panel.active_project_id = pid.clone();
                panel.expanded_project_ids.insert(pid.clone());
            }
            panel.lines = Arc::new(Vec::new());
            panel.event_cursor = 0;
            panel.polling_session = false;
            panel.busy = false;
            panel.chat_messages.clear();
            panel.resume_id = None;
            panel.active_session_id = None;
        }
        if Self::is_local_sidebar_session(&session_id) {
            ctx.notify();
            return;
        }
        self.load_session_transcript(session_id, session.running, ctx);
    }

    fn new_project(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_create = Some(ProjectCreateState::new_type_select());
            panel.project_row_menu = None;
        }
        ctx.notify();
    }

    fn new_standalone_chat(&mut self, ctx: &mut ViewContext<Self>) {
        {
            let mut panel = self.state.lock().expect("agent panel state");
            Self::insert_new_session(&mut panel, None);
        }
        self.bump();
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn new_project_thread(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.active_project_id = project_id.clone();
            panel.expanded_project_ids.insert(project_id.clone());
            Self::insert_new_session(&mut panel, Some(project_id));
        }
        self.bump();
        sync_caret_blink(self, ctx);
        ctx.notify();
    }

    fn new_thread(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_standalone_chat(ctx);
    }

    fn open_project_create_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_create = Some(ProjectCreateState::new_type_select());
            panel.project_row_menu = None;
            panel.project_delete = None;
        }
        ctx.notify();
    }

    fn close_project_create_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_create = None;
        }
        ctx.notify();
    }

    fn project_create_next(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            if let Some(create) = panel.project_create.as_mut() {
                create.step = ProjectCreateStep::Name;
                create.name_focused = true;
                create.name_invalid = false;
            }
        }
        ctx.notify();
    }

    fn project_create_use_existing_folder(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(path) = Self::pick_project_folder("选择项目文件夹") else {
            return;
        };
        if !path.is_dir() {
            if let Ok(mut panel) = self.state.lock() {
                panel.status = format!("项目目录不存在：{}", path.display());
            }
            ctx.notify();
            return;
        }
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("New project")
            .to_string();
        self.insert_created_project(label, path, ctx);
    }

    fn default_projects_parent(data_dir: &std::path::Path) -> PathBuf {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home).join("Projects");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Projects");
        }
        data_dir.join("Projects")
    }

    fn insert_created_project(
        &mut self,
        label: String,
        folder_path: PathBuf,
        ctx: &mut ViewContext<Self>,
    ) {
        let data_dir = self.core.data_dir();
        if let Ok(mut panel) = self.state.lock() {
            let id = format!("project-{}", Self::unix_now_secs());
            panel.projects.insert(
                0,
                sidebar::AgentProject {
                    id: id.clone(),
                    label,
                    time: "刚刚".into(),
                    folder_path,
                },
            );
            panel.expanded_project_ids.insert(id.clone());
            panel.active_project_id = id;
            panel.project_create = None;
            panel.status = "项目已创建".into();
            Self::persist_projects(&panel, &data_dir);
        }
        ctx.notify();
    }

    fn project_create_submit(&mut self, ctx: &mut ViewContext<Self>) {
        let data_dir = self.core.data_dir();
        let (label, folder_path) = {
            let mut panel = self.state.lock().expect("agent panel state");
            let Some(create) = panel.project_create.as_mut() else {
                return;
            };
            let name = create.name.trim().to_string();
            if name.is_empty() {
                create.name_invalid = true;
                drop(panel);
                ctx.notify();
                return;
            }
            let parent = Self::default_projects_parent(&data_dir);
            let path = parent.join(&name);
            if let Err(err) = std::fs::create_dir_all(&path) {
                panel.status = format!("无法创建项目目录：{err}");
                drop(panel);
                ctx.notify();
                return;
            }
            (name, path)
        };
        self.insert_created_project(label, folder_path, ctx);
    }

    fn toggle_project_expanded(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            if panel.expanded_project_ids.contains(&project_id) {
                panel.expanded_project_ids.remove(&project_id);
            } else {
                panel.expanded_project_ids.insert(project_id);
            }
            let data_dir = self.core.data_dir();
            Self::persist_projects(&panel, &data_dir);
        }
        ctx.notify();
    }

    fn open_project_row_menu(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            let next = if panel.project_row_menu.as_deref() == Some(project_id.as_str()) {
                None
            } else {
                Some(project_id.clone())
            };
            panel.active_project_id = project_id;
            panel.project_row_menu = next;
            panel.chats_menu_open = false;
            panel.session_context_menu = None;
        }
        ctx.notify();
    }

    fn open_project_delete_modal(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            if !panel.projects.iter().any(|p| p.id == project_id) {
                return;
            }
            panel.project_row_menu = None;
            panel.chats_menu_open = false;
            panel.session_context_menu = None;
            panel.project_delete = Some(ProjectDeleteState::new(project_id));
        }
        ctx.notify();
    }

    fn close_project_delete_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_delete = None;
        }
        ctx.notify();
    }

    fn set_project_delete_mode(&mut self, delete_local: bool, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            let project_id = match panel.project_delete.as_ref() {
                Some(state) => state.project_id.clone(),
                None => return,
            };
            if delete_local {
                let has_path = panel
                    .projects
                    .iter()
                    .find(|p| p.id == project_id)
                    .map(|p| !p.folder_path.as_os_str().is_empty())
                    .unwrap_or(false);
                if !has_path {
                    return;
                }
            }
            if let Some(state) = panel.project_delete.as_mut() {
                state.delete_local = delete_local;
            }
        }
        ctx.notify();
    }

    fn confirm_delete_project(&mut self, ctx: &mut ViewContext<Self>) {
        let (project_id, delete_local) = {
            let panel = self.state.lock().expect("agent panel state");
            let Some(state) = panel.project_delete.as_ref() else {
                return;
            };
            (state.project_id.clone(), state.delete_local)
        };
        self.delete_project(project_id, delete_local, ctx);
    }

    fn rename_project(&mut self, project_id: String, ctx: &mut ViewContext<Self>) {
        let new_label = {
            let panel = self.state.lock().expect("agent panel state");
            panel
                .projects
                .iter()
                .find(|p| p.id == project_id)
                .map(|p| format!("{}_copy", p.label))
                .unwrap_or_else(|| "重命名项目".into())
        };
        if let Ok(mut panel) = self.state.lock() {
            if let Some(project) = panel.projects.iter_mut().find(|p| p.id == project_id) {
                project.label = new_label;
            }
            let data_dir = self.core.data_dir();
            Self::persist_projects(&panel, &data_dir);
        }
        ctx.notify();
    }

    fn persist_archived_ids(&self, archived_ids: &HashSet<String>) {
        sidebar::save_archived_ids(&self.core.data_dir(), archived_ids);
    }

    fn clear_sidebar_hover(panel: &mut PanelState) {
        panel.sidebar_hover = None;
    }

    fn dismiss_sidebar_menus(&mut self, ctx: &mut ViewContext<Self>) {
        let mut changed = false;
        if let Ok(mut panel) = self.state.lock() {
            if panel.project_row_menu.is_some() || panel.chats_menu_open {
                panel.project_row_menu = None;
                panel.chats_menu_open = false;
                Self::clear_sidebar_hover(&mut panel);
                changed = true;
            }
        }
        if changed {
            ctx.notify();
        }
    }

    fn close_session_context_menu(&mut self, ctx: &mut ViewContext<Self>) {
        let mut changed = false;
        if let Ok(mut panel) = self.state.lock() {
            if panel.session_context_menu.is_some() {
                panel.session_context_menu = None;
                changed = true;
            }
        }
        if changed {
            ctx.notify();
        }
    }

    fn dismiss_all_overlays(&mut self, ctx: &mut ViewContext<Self>) {
        self.close_session_context_menu(ctx);
        self.dismiss_sidebar_menus(ctx);
        self.dismiss_composer_menus(ctx);
    }

    fn toggle_chats_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_row_menu = None;
            panel.session_context_menu = None;
            panel.chats_menu_open = !panel.chats_menu_open;
            panel.access_menu_open = false;
            panel.model_menu_open = false;
            Self::clear_sidebar_hover(&mut panel);
        }
        ctx.notify();
    }

    pub fn restore_archived_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        self.restore_session(session_id, ctx);
    }

    pub fn delete_archived_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        self.delete_session(session_id, ctx);
    }

    fn next_active_session_in_project(
        panel: &PanelState,
        project_id: &str,
        exclude_id: Option<&str>,
    ) -> Option<String> {
        panel
            .sidebar_sessions
            .iter()
            .find(|s| {
                s.project_id.as_deref() == Some(project_id)
                    && !panel.archived_ids.contains(&s.id)
                    && exclude_id.map(|id| s.id != id).unwrap_or(true)
            })
            .map(|s| s.id.clone())
    }

    fn next_active_standalone_session(
        panel: &PanelState,
        exclude_id: Option<&str>,
    ) -> Option<String> {
        panel
            .sidebar_sessions
            .iter()
            .find(|s| {
                sidebar::is_standalone(s)
                    && !panel.archived_ids.contains(&s.id)
                    && exclude_id.map(|id| s.id != id).unwrap_or(true)
            })
            .map(|s| s.id.clone())
    }

    fn next_session_after_archive(panel: &PanelState, session: &sidebar::AgentSession) -> Option<String> {
        if let Some(pid) = &session.project_id {
            Self::next_active_session_in_project(panel, pid, Some(&session.id))
        } else {
            Self::next_active_standalone_session(panel, Some(&session.id))
        }
    }

    fn archive_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        let switch_to = {
            let mut panel = self.state.lock().expect("agent panel state");
            if panel.archived_ids.contains(&session_id) {
                return;
            }
            let archived_session = panel
                .sidebar_sessions
                .iter()
                .find(|s| s.id == session_id)
                .cloned();
            panel.archived_ids.insert(session_id.clone());
            if let Some(session) = panel.sidebar_sessions.iter_mut().find(|s| s.id == session_id) {
                session.running = false;
            }
            if panel.active_sidebar_session_id == session_id {
                panel.polling_session = false;
                panel.busy = false;
            }
            panel.session_context_menu = None;
            panel.chats_menu_open = false;
            let was_active = panel.active_sidebar_session_id == session_id;
            let archived = panel.archived_ids.clone();
            self.persist_archived_ids(&archived);
            let switch_to = if was_active {
                archived_session
                    .as_ref()
                    .and_then(|s| Self::next_session_after_archive(&panel, s))
            } else {
                None
            };
            if let Some(session) = archived_session {
                let snapshot = sidebar::ArchivedSessionSnapshot {
                    id: session.id,
                    label: session.label,
                    project_id: session.project_id,
                    time: session.time,
                };
                sidebar::upsert_archived_snapshot(&self.core.data_dir(), &snapshot);
            }
            switch_to
        };
        if let Some(next_id) = switch_to {
            self.select_session(next_id, ctx);
        } else if self
            .state
            .lock()
            .map(|p| p.active_sidebar_session_id == session_id)
            .unwrap_or(false)
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.active_sidebar_session_id.clear();
            panel.thread_title = "新会话".into();
            panel.lines = Arc::new(Vec::new());
            panel.active_session_id = None;
            ctx.notify();
        } else {
            ctx.notify();
        }
    }

    fn restore_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.archived_ids.remove(&session_id);
            panel.session_context_menu = None;
            let archived = panel.archived_ids.clone();
            self.persist_archived_ids(&archived);
            sidebar::remove_archived_snapshot(&self.core.data_dir(), &session_id);
        }
        self.select_session(session_id, ctx);
    }

    fn delete_session(&mut self, session_id: String, ctx: &mut ViewContext<Self>) {
        if !self
            .state
            .lock()
            .map(|p| p.archived_ids.contains(&session_id))
            .unwrap_or(false)
        {
            return;
        }
        let was_active = self
            .state
            .lock()
            .map(|p| p.active_sidebar_session_id == session_id)
            .unwrap_or(false);
        let project_id = self
            .state
            .lock()
            .ok()
            .and_then(|p| {
                p.sidebar_sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .and_then(|s| s.project_id.clone())
            })
            .unwrap_or_default();
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.archived_ids.remove(&session_id);
            panel.sidebar_sessions.retain(|s| s.id != session_id);
            panel.session_context_menu = None;
            let archived = panel.archived_ids.clone();
            self.persist_archived_ids(&archived);
            sidebar::remove_archived_snapshot(&self.core.data_dir(), &session_id);
            if was_active {
                panel.active_sidebar_session_id.clear();
                panel.thread_title = "新会话".into();
                panel.lines = Arc::new(Vec::new());
                panel.active_session_id = None;
            }
        }
        if was_active {
            if let Some(next_id) = self.state.lock().ok().and_then(|panel| {
                if project_id.is_empty() {
                    Self::next_active_standalone_session(&panel, None)
                } else {
                    Self::next_active_session_in_project(&panel, &project_id, None)
                }
            }) {
                self.select_session(next_id, ctx);
            } else {
                ctx.notify();
            }
        } else {
            ctx.notify();
        }
    }

    fn delete_active_standalone_chat(&mut self, ctx: &mut ViewContext<Self>) {
        let session_id = self
            .state
            .lock()
            .map(|p| p.active_sidebar_session_id.clone())
            .unwrap_or_default();
        if session_id.is_empty() {
            return;
        }
        let is_standalone = self
            .state
            .lock()
            .ok()
            .and_then(|p| {
                p.sidebar_sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .map(sidebar::is_standalone)
            })
            .unwrap_or(false);
        if !is_standalone {
            return;
        }
        {
            let mut panel = self.state.lock().expect("agent panel state");
            panel.sidebar_sessions.retain(|s| s.id != session_id);
            panel.archived_ids.remove(&session_id);
            panel.chats_menu_open = false;
            if panel.active_sidebar_session_id == session_id {
                panel.active_sidebar_session_id.clear();
                panel.thread_title = "新会话".into();
                panel.lines = Arc::new(Vec::new());
                panel.chat_messages.clear();
                panel.resume_id = None;
                panel.active_session_id = None;
                panel.busy = false;
            }
            let archived = panel.archived_ids.clone();
            self.persist_archived_ids(&archived);
        }
        ctx.notify();
    }

    fn delete_project(
        &mut self,
        project_id: String,
        delete_local: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let target_id = if project_id.is_empty() {
            self.state
                .lock()
                .map(|p| p.active_project_id.clone())
                .unwrap_or_default()
        } else {
            project_id
        };
        if target_id.is_empty() {
            return;
        }

        let folder_path = {
            let panel = self.state.lock().expect("agent panel state");
            panel
                .projects
                .iter()
                .find(|p| p.id == target_id)
                .map(|p| p.folder_path.clone())
        };

        let Some(folder_path) = folder_path else {
            return;
        };

        if delete_local && !folder_path.as_os_str().is_empty() {
            if folder_path.is_dir() {
                if let Err(err) = std::fs::remove_dir_all(&folder_path) {
                    if let Ok(mut panel) = self.state.lock() {
                        panel.status = format!("无法删除本地文件夹：{err}");
                        // Keep modal open so the user can retry or switch to unlink.
                    }
                    ctx.notify();
                    return;
                }
            } else if folder_path.exists() {
                if let Ok(mut panel) = self.state.lock() {
                    panel.status = format!(
                        "无法删除本地路径（不是文件夹）：{}",
                        folder_path.display()
                    );
                }
                ctx.notify();
                return;
            }
        }

        let should_switch = {
            let mut panel = self.state.lock().expect("agent panel state");
            let was_active = panel.active_project_id == target_id;
            panel.projects.retain(|p| p.id != target_id);
            panel
                .sidebar_sessions
                .retain(|s| s.project_id.as_deref() != Some(target_id.as_str()));
            panel.expanded_project_ids.remove(&target_id);
            panel.project_row_menu = None;
            panel.project_delete = None;
            panel.status = if delete_local && !folder_path.as_os_str().is_empty() {
                format!("已删除项目并移除本地文件夹 {}", folder_path.display())
            } else {
                "已去掉项目引用 · 本机文件保留".into()
            };
            let data_dir = self.core.data_dir();
            Self::persist_projects(&panel, &data_dir);
            if was_active {
                panel.active_project_id =
                    panel.projects.first().map(|p| p.id.clone()).unwrap_or_default();
                true
            } else {
                false
            }
        };
        if should_switch {
            let next_project = self
                .state
                .lock()
                .map(|p| p.active_project_id.clone())
                .unwrap_or_default();
            self.select_project(next_project, ctx);
        } else {
            ctx.notify();
        }
    }

    fn copy_user_prompt(&mut self, ctx: &mut ViewContext<Self>) {
        let text = {
            let panel = self.state.lock().expect("agent panel state");
            panel
                .lines
                .iter()
                .find(|line| line.channel == "user")
                .map(|line| line.text.clone())
                .or_else(|| {
                    if panel.active_sidebar_session_id.is_empty() {
                        None
                    } else {
                        panel
                            .sidebar_sessions
                            .iter()
                            .find(|s| s.id == panel.active_sidebar_session_id)
                            .map(|s| s.prompt.clone())
                            .filter(|p| !p.trim().is_empty())
                    }
                })
        };
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            if write_clipboard_text(&text).is_ok() {
                if let Ok(mut panel) = self.state.lock() {
                    panel.status = "已复制指令".into();
                }
            }
        }
        ctx.notify();
    }

    fn dismiss_composer_menus(&mut self, ctx: &mut ViewContext<Self>) {
        let mut changed = false;
        if let Ok(mut panel) = self.state.lock() {
            if panel.access_menu_open {
                panel.access_menu_open = false;
                changed = true;
            }
            if panel.model_menu_open {
                panel.model_menu_open = false;
                changed = true;
            }
            if panel.add_menu_open {
                panel.add_menu_open = false;
                changed = true;
            }
        }
        if changed {
            ctx.notify();
        }
    }

    fn toggle_add_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_row_menu = None;
            panel.chats_menu_open = false;
            panel.session_context_menu = None;
            panel.access_menu_open = false;
            panel.model_menu_open = false;
            panel.add_menu_open = !panel.add_menu_open;
        }
        ctx.notify();
    }

    fn open_files_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.add_menu_open = false;
            panel.media_modal = None;
            panel.files_modal = Some(FilesModalState::new());
        }
        ctx.notify();
    }

    fn close_files_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.files_modal = None;
        }
        ctx.notify();
    }

    fn select_demo_file(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            if let Some(modal) = panel.files_modal.as_mut() {
                if index < DEMO_FILES.len() {
                    modal.selected = index;
                }
            }
        }
        ctx.notify();
    }

    fn confirm_files_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            let Some(modal) = panel.files_modal.as_ref() else {
                return;
            };
            let Some(item) = DEMO_FILES.get(modal.selected) else {
                return;
            };
            let path = item.path.to_string();
            if !panel.attachments.iter().any(|a| a.path == path) {
                panel.attachments.push(ComposerAttachment {
                    name: item.name.to_string(),
                    path,
                    size: item.size.to_string(),
                    is_folder: item.is_folder,
                });
            }
            panel.files_modal = None;
        }
        ctx.notify();
    }

    fn open_media_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.add_menu_open = false;
            panel.files_modal = None;
            panel.media_modal = Some(MediaModalState::new());
        }
        ctx.notify();
    }

    fn close_media_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.media_modal = None;
        }
        ctx.notify();
    }

    fn select_demo_media(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            if let Some(modal) = panel.media_modal.as_mut() {
                if index < DEMO_MEDIA.len() {
                    modal.selected = index;
                }
            }
        }
        ctx.notify();
    }

    fn confirm_media_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            let Some(modal) = panel.media_modal.as_ref() else {
                return;
            };
            let Some(item) = DEMO_MEDIA.get(modal.selected) else {
                return;
            };
            let path = item.name.to_string();
            if !panel.attachments.iter().any(|a| a.path == path) {
                panel.attachments.push(ComposerAttachment {
                    name: item.name.to_string(),
                    path,
                    size: item.size.to_string(),
                    is_folder: false,
                });
            }
            panel.media_modal = None;
        }
        ctx.notify();
    }

    fn toggle_goal_mode(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.goal_mode = !panel.goal_mode;
            panel.add_menu_open = false;
        }
        ctx.notify();
    }

    fn toggle_plan_mode(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.plan_mode = !panel.plan_mode;
            panel.add_menu_open = false;
        }
        ctx.notify();
    }

    fn toggle_access_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_row_menu = None;
            panel.chats_menu_open = false;
            panel.session_context_menu = None;
            panel.model_menu_open = false;
            panel.add_menu_open = false;
            panel.access_menu_open = !panel.access_menu_open;
        }
        ctx.notify();
    }

    fn toggle_model_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.project_row_menu = None;
            panel.chats_menu_open = false;
            panel.session_context_menu = None;
            panel.access_menu_open = false;
            panel.add_menu_open = false;
            panel.model_menu_open = !panel.model_menu_open;
        }
        ctx.notify();
    }

    fn select_access_mode(&mut self, mode: AgentAccessMode, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.access_menu_open = false;
            panel.model_menu_open = false;
            panel.add_menu_open = false;
        }
        if self.access_mode == mode {
            ctx.notify();
            return;
        }
        self.access_mode = mode;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ =
                core.block_on(async { warp_embed_prefs::set_access_mode(&data_dir, mode).await });
        });
        ctx.notify();
    }

    fn select_model_rate(&mut self, rate: AgentModelRate, ctx: &mut ViewContext<Self>) {
        if let Ok(mut panel) = self.state.lock() {
            panel.model_menu_open = false;
            panel.access_menu_open = false;
        }
        if self.model_rate == rate {
            ctx.notify();
            return;
        }
        self.model_rate = rate;
        let data_dir = self.core.data_dir();
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ =
                core.block_on(async { warp_embed_prefs::set_model_rate(&data_dir, rate).await });
        });
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
            state.active_sidebar_session_id.clear();
            state.thread_title = "新会话".into();
        }
        self.bump();
        ctx.notify();
    }

    fn update_status_line(&self, ctx: &mut ViewContext<Self>) {
        let agent_label = "Codex";
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
                view.reload_sidebar_sessions(ctx);
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
            Self::ensure_sidebar_session_for_send(&mut state);
            state.busy = true;
            state.draft.clear();
            let title = Self::session_label_from_dto(&AgentSessionDto {
                id: String::new(),
                status: String::new(),
                prompt: Some(text.clone()),
                cwd: None,
                started_at: Self::unix_now_secs(),
                ended_at: None,
                log_path: None,
            });
            state.thread_title = title.clone();
            let active_session_id = state.active_sidebar_session_id.clone();
            if let Some(session) = state
                .sidebar_sessions
                .iter_mut()
                .find(|s| s.id == active_session_id)
            {
                session.label = title;
                session.prompt = text.clone();
            }
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
        let access_mode = self.access_mode;
        let generation_notify = self.generation_notify_tx.clone();
        ctx.spawn(
            async move {
                let data_dir = core.data_dir();
                let cwd = {
                    let state = shared_async.lock().expect("agent panel state");
                    let session_id = state.active_sidebar_session_id.clone();
                    let project_id = state
                        .sidebar_sessions
                        .iter()
                        .find(|s| s.id == session_id)
                        .and_then(|s| s.project_id.clone());
                    let folder = project_id.as_ref().and_then(|pid| {
                        state
                            .projects
                            .iter()
                            .find(|p| p.id == *pid)
                            .map(|p| p.folder_path.clone())
                    });
                    (session_id, project_id, folder)
                };
                let resolved_cwd = match (&cwd.0, &cwd.1, &cwd.2) {
                    (_, Some(_), Some(path)) => {
                        if path.is_dir() {
                            Ok(Some(path.display().to_string()))
                        } else {
                            Err(format!("项目目录不存在：{}", path.display()))
                        }
                    }
                    (_, None, _) if !cwd.0.is_empty() => {
                        let workspace = resolve_agent_workspace_cwd(&data_dir).await;
                        Ok(Some(workspace.display().to_string()))
                    }
                    _ => Ok(None),
                };
                if let Err(err) = &resolved_cwd {
                    return Err(err.clone());
                }
                let params = {
                    let state = shared_async.lock().expect("agent panel state");
                    AgentLlmChatParams {
                        messages: state.chat_messages.clone(),
                        cwd: resolved_cwd.ok().flatten(),
                        thread_id: state.resume_id.clone(),
                        agent_id: None,
                        force_full_auto: access_mode.force_full_auto(),
                    }
                };
                let state = core.app_state();
                let stream_shared = Arc::clone(&shared_async);
                let stream_notify = generation_notify.clone();
                agent_chat_with_codex_fallback(params, state, move |event| {
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
            },
            move |_view, output, ctx| {
                let mut panel = shared_callback.lock().expect("agent panel state");
                match output {
                    Ok(result) => {
                        if result.backend == AgentTurnBackend::CursorFallback {
                            if let Some(reason) = &result.codex_error {
                                Self::push_cursor_fallback_status(&mut panel, reason);
                            }
                        }
                        Self::append_chat_completion_transcript(
                            &mut panel,
                            result.backend,
                            &result.reply.content,
                        );
                        panel.chat_messages.push(AgentLlmChatMessage {
                            role: "assistant".into(),
                            content: result.reply.content,
                        });
                        panel.resume_id = result.reply.thread_id;
                        panel.status = match result.backend {
                            AgentTurnBackend::Codex => {
                                format!("完成 · {}", result.reply.model)
                            }
                            AgentTurnBackend::CursorFallback => "完成 · Cursor（后备）".into(),
                        };
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
        let shared_callback = Arc::clone(&self.state);
        let generation = Arc::clone(&self.generation);
        ctx.spawn(
            async move {
                let data_dir = core.data_dir();
                let cwd_hint = {
                    let panel = shared.lock().expect("agent panel state");
                    let session_id = panel.active_sidebar_session_id.clone();
                    let project_id = panel
                        .sidebar_sessions
                        .iter()
                        .find(|s| s.id == session_id)
                        .and_then(|s| s.project_id.clone());
                    let folder = project_id.as_ref().and_then(|pid| {
                        panel
                            .projects
                            .iter()
                            .find(|p| p.id == *pid)
                            .map(|p| p.folder_path.clone())
                    });
                    (session_id, project_id, folder)
                };
                let resolved_cwd = match (&cwd_hint.0, &cwd_hint.1, &cwd_hint.2) {
                    (_, Some(_), Some(path)) => {
                        if path.is_dir() {
                            Ok(Some(path.display().to_string()))
                        } else {
                            Err(format!("项目目录不存在：{}", path.display()))
                        }
                    }
                    (_, None, _) if !cwd_hint.0.is_empty() => {
                        let workspace = resolve_agent_workspace_cwd(&data_dir).await;
                        Ok(Some(workspace.display().to_string()))
                    }
                    _ => Ok(None),
                };
                if let Err(err) = &resolved_cwd {
                    return Err(err.clone());
                }
                let cwd_str = resolved_cwd.ok().flatten();
                let request = AgentStartRequest {
                    prompt,
                    cwd: cwd_str,
                };
                let state = core.app_state();
                agent_start_session_with_codex_fallback(request, state).await
            },
            move |view, output, ctx| {
                let mut panel = shared_callback.lock().expect("agent panel state");
                let mut started = false;
                match output {
                    Ok(result) => {
                        if result.backend == AgentTurnBackend::CursorFallback {
                            if let Some(reason) = &result.codex_error {
                                Self::push_cursor_fallback_status(&mut panel, reason);
                            }
                        }
                        let session = result.session;
                        Self::push_line(
                            &mut panel,
                            TranscriptLine {
                                channel: "status".into(),
                                text: format!("任务已启动 · session {}", session.id),
                                level: "info".into(),
                            },
                        );
                        panel.active_session_id = Some(session.id.clone());
                        panel.active_sidebar_session_id = session.id.clone();
                        panel.event_cursor = 0;
                        panel.polling_session = true;
                        panel.status = "任务运行中…".into();
                        let project_id = if panel.active_project_id.is_empty() {
                            None
                        } else {
                            Some(panel.active_project_id.clone())
                        };
                        let mapped = Self::map_session_dto(session, project_id);
                        panel.thread_title = mapped.label.clone();
                        Self::upsert_sidebar_session(&mut panel, mapped);
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
        let shared = Arc::clone(&self.state);
        ctx.spawn(
            async move {
                let app = core.runtime().ctx.clone();
                let state = core.app_state();
                agent_launch_terminal_with_codex_fallback(&app, None, state).await
            },
            move |_view, output, ctx| {
                let mut state = shared.lock().expect("agent panel state");
                match output {
                    Ok(result) => {
                        if result.backend == AgentTurnBackend::CursorFallback {
                            let reason = result
                                .codex_error
                                .as_deref()
                                .unwrap_or("Codex 终端不可用");
                            state.status =
                                format!("已改用 Cursor 终端（后备）: {reason}");
                        }
                    }
                    Err(err) => {
                        state.status = format!("无法启动终端: {err}");
                    }
                }
                ctx.notify();
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

    fn composer_bar(
        &self,
        busy: bool,
        draft_empty: bool,
        access_mode: AgentAccessMode,
        model_rate: AgentModelRate,
    ) -> Box<dyn Element> {
        let access_chip = self.composer_labeled_chip(
            access_label(access_mode),
            Some(AgentPanelAction::ToggleAccessMenu),
            Some("agent-warn.svg"),
            true,
        );
        let model_chip = self.composer_labeled_chip(
            composer_model_chip_label(model_rate),
            Some(AgentPanelAction::ToggleModelMenu),
            None,
            true,
        );
        let add_btn = self.composer_icon_chip(
            "agent-plus.svg",
            theme::muted(),
            Some(AgentPanelAction::ToggleAddMenu),
        );
        let bar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(add_btn)
            .with_child(access_chip)
            .with_child(Expanded::new(1.0, Flex::row().finish()).finish())
            .with_child(model_chip)
            .with_child(self.stop_button(busy, draft_empty));
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

        let draft_empty = draft.trim().is_empty();
        TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(AgentPanelAction::TextFieldEdit(action));
        })
        .focused(input_focused)
        .disabled(busy)
        .ime_preedit(!marked.is_empty())
        .on_keydown({
            let busy = busy;
            let draft_empty = draft_empty;
            move |ctx, keystroke| {
                if busy {
                    return DispatchEventResult::PropagateToParent;
                }
                match keystroke.key.as_str() {
                    "enter" | "return" => {
                        if keystroke.shift {
                            ctx.dispatch_typed_action(AgentPanelAction::TextFieldEdit(
                                TextFieldEditAction::InsertNewline,
                            ));
                        } else if !draft_empty {
                            ctx.dispatch_typed_action(AgentPanelAction::Send);
                        }
                        DispatchEventResult::StopPropagation
                    }
                    "escape" => {
                        ctx.dispatch_typed_action(AgentPanelAction::ClearAndUnfocus);
                        DispatchEventResult::StopPropagation
                    }
                    "tab" => {
                        ctx.dispatch_typed_action(AgentPanelAction::ToggleComposerFocus);
                        DispatchEventResult::StopPropagation
                    }
                    _ => DispatchEventResult::PropagateToParent,
                }
            }
        })
        .finish()
    }

    fn composer_bottom_layer(
        &self,
        folder: Option<(String, String)>,
        inner: Box<dyn Element>,
    ) -> Box<dyn Element> {
        // HTML `.agent-composer-wrap`: composer card first, then `#agent-composer-folder`.
        let mut col = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        col.add_child(inner);
        if let Some((name, path)) = folder {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max);
            row.add_child(icons::agent_icon("agent-folder.svg", theme::accent_cool()));
            row.add_child(
                Container::new(
                    ui_text::body(name, self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_left(8.0)
                .finish(),
            );
            let _ = path;
            col.add_child(
                ConstrainedBox::new(Empty::new().finish())
                    .with_height(COMPOSER_FOLDER_GAP)
                    .finish(),
            );
            col.add_child(
                ConstrainedBox::new(
                    Container::new(row.finish())
                        .with_padding_left(14.0)
                        .with_padding_right(14.0)
                        .with_padding_top(8.0)
                        .with_padding_bottom(8.0)
                        .with_background(theme::panel_elevated())
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(warpui::elements::CornerRadius::with_all(
                            warpui::elements::Radius::Pixels(12.0),
                        ))
                        .finish(),
                )
                .with_max_width(AGENT_THREAD_MAX_WIDTH)
                .with_width(AGENT_THREAD_MAX_WIDTH)
                .with_min_height(COMPOSER_FOLDER_BAR_HEIGHT)
                .finish(),
            );
        }
        let wrap = Container::new(center_composer_width(col.finish()))
            .with_padding_left(24.0)
            .with_padding_right(24.0)
            .with_padding_top(8.0)
            .with_padding_bottom(16.0)
            .finish();
        Align::new(wrap).bottom_center().finish()
    }

    fn composer_center_layer(&self, inner: Box<dyn Element>) -> Box<dyn Element> {
        let wrap = Container::new(center_composer_width(inner))
            .with_padding_left(24.0)
            .with_padding_right(24.0)
            .finish();
        Align::new(wrap).finish()
    }

    /// Access / model popovers for the bottom composer (selected session).
    /// Anchors with `composer_bottom_layer` + `bottom_*` so menus sit above chips.
    /// When `folder_below` is set, lift menus by the folder bar + gap so they still
    /// align to the composer bar (HTML: folder sits under the card).
    fn render_composer_popovers(
        &self,
        access_mode: AgentAccessMode,
        model_rate: AgentModelRate,
        access_menu_open: bool,
        model_menu_open: bool,
        add_menu_open: bool,
        goal_mode: bool,
        plan_mode: bool,
        folder_below: bool,
    ) -> Box<dyn Element> {
        let bar_lift = if folder_below {
            COMPOSER_BAR_LIFT + COMPOSER_FOLDER_BAR_HEIGHT + COMPOSER_FOLDER_GAP
        } else {
            COMPOSER_BAR_LIFT
        };
        let mut stack = Stack::new();
        self.push_composer_popover_aligns(
            &mut stack,
            access_mode,
            model_rate,
            access_menu_open,
            model_menu_open,
            add_menu_open,
            goal_mode,
            plan_mode,
            bar_lift,
        );
        self.composer_bottom_layer(None, stack.finish())
    }

    fn render_idle_composer_popovers(
        &self,
        card_height: f32,
        access_mode: AgentAccessMode,
        model_rate: AgentModelRate,
        access_menu_open: bool,
        model_menu_open: bool,
        add_menu_open: bool,
        goal_mode: bool,
        plan_mode: bool,
    ) -> Box<dyn Element> {
        let mut stack = Stack::new();
        stack.add_child(Empty::new().finish());
        self.push_composer_popover_aligns(
            &mut stack,
            access_mode,
            model_rate,
            access_menu_open,
            model_menu_open,
            add_menu_open,
            goal_mode,
            plan_mode,
            COMPOSER_BAR_LIFT,
        );
        let anchor = ConstrainedBox::new(stack.finish())
            .with_width(AGENT_THREAD_MAX_WIDTH)
            .with_height(card_height)
            .finish();
        self.composer_center_layer(anchor)
    }

    fn push_composer_popover_aligns(
        &self,
        stack: &mut Stack,
        access_mode: AgentAccessMode,
        model_rate: AgentModelRate,
        access_menu_open: bool,
        model_menu_open: bool,
        add_menu_open: bool,
        goal_mode: bool,
        plan_mode: bool,
        bar_lift: f32,
    ) {
        if add_menu_open {
            stack.add_child(
                Align::new(
                    Container::new(composer_add::render_add_menu(
                        self.font, goal_mode, plan_mode,
                    ))
                    .with_margin_left(ADD_POPOVER_INSET_LEFT)
                    .with_margin_bottom(bar_lift)
                    .finish(),
                )
                .bottom_left()
                .finish(),
            );
        }
        if access_menu_open {
            stack.add_child(
                Align::new(
                    Container::new(composer_menus::render_access_menu(self.font, access_mode))
                        .with_margin_left(ACCESS_POPOVER_INSET_LEFT)
                        .with_margin_bottom(bar_lift)
                        .finish(),
                )
                .bottom_left()
                .finish(),
            );
        }
        if model_menu_open {
            stack.add_child(
                Align::new(
                    Container::new(composer_menus::render_model_rate_menu(self.font, model_rate))
                        .with_margin_bottom(bar_lift)
                        .finish(),
                )
                .bottom_right()
                .finish(),
            );
        }
    }

    fn overlay_scrim(&self) -> Box<dyn Element> {
        EventHandler::new(
            Container::new(Flex::row().finish())
                .with_background(ColorU::new(8, 7, 11, 150))
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::CloseProjectCreateModal);
            ctx.dispatch_typed_action(AgentPanelAction::CloseSessionContextMenu);
            ctx.dispatch_typed_action(AgentPanelAction::DismissSidebarMenus);
            ctx.dispatch_typed_action(AgentPanelAction::DismissComposerMenus);
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn stop_button(&self, busy: bool, draft_empty: bool) -> Box<dyn Element> {
        const BTN_SIZE: f32 = 32.0;
        const BTN_RADIUS: f32 = 16.0;

        let void = theme::canvas();
        let icon = if busy {
            Container::new(
                ConstrainedBox::new(Flex::row().finish())
                    .with_width(10.0)
                    .with_height(10.0)
                    .finish(),
            )
            .with_background(void)
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(2.0),
            ))
            .finish()
        } else {
            icons::icon("agent-send.svg", 20.0, void)
        };
        let action = if busy {
            AgentPanelAction::Stop
        } else if !draft_empty {
            AgentPanelAction::Send
        } else {
            AgentPanelAction::FocusInput
        };
        let btn = Container::new(
            ConstrainedBox::new(Align::new(icon).finish())
                .with_width(BTN_SIZE)
                .with_height(BTN_SIZE)
                .finish(),
        )
        .with_background(theme::accent())
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(BTN_RADIUS),
        ))
        .finish();
        Container::new(
            ConstrainedBox::new(
                EventHandler::new(btn)
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(action.clone());
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_width(BTN_SIZE)
            .with_height(BTN_SIZE)
            .with_min_width(BTN_SIZE)
            .with_min_height(BTN_SIZE)
            .finish(),
        )
        .with_margin_left(8.0)
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
        let thinking = state.busy && state.polling_session;
        let access_menu_open = state.access_menu_open;
        let model_menu_open = state.model_menu_open;
        let add_menu_open = state.add_menu_open;
        let plan_mode = state.plan_mode;
        let goal_mode = state.goal_mode;
        let attachments = state.attachments.clone();
        let files_modal = state.files_modal.clone();
        let media_modal = state.media_modal.clone();
        let access_mode = self.access_mode;
        let model_rate = self.model_rate;
        let projects = state.projects.clone();
        let sidebar_sessions = state.sidebar_sessions.clone();
        let expanded_project_ids = state.expanded_project_ids.clone();
        let active_project_id = state.active_project_id.clone();
        let active_sidebar_session_id = state.active_sidebar_session_id.clone();
        let thread_title = state.thread_title.clone();
        let sidebar_search = state.sidebar_search.clone();
        let sidebar_search_focused = state.sidebar_search_focused;
        let archived_ids = state.archived_ids.clone();
        let project_row_menu = state.project_row_menu.clone();
        let chats_menu_open = state.chats_menu_open;
        let session_context_menu = state.session_context_menu.clone();
        let project_create = state.project_create.clone();
        let project_delete = state.project_delete.clone();
        let sidebar_hover = state.sidebar_hover.clone();
        let project_head_hover = state.project_head_hover.clone();
        drop(state);

        let input_border = if input_focused {
            theme::accent_cool()
        } else {
            theme::border_bright()
        };

        let placeholder = if busy {
            "执行中…"
        } else if Self::is_idle_composer_state(&active_sidebar_session_id) {
            "描述你想让 AI 执行的任务…"
        } else {
            "输入后续修改或追问…"
        };

        let input_height = multiline_input::box_height(&draft, multiline_input::DEFAULT_COLS);

        let transcript_model = TranscriptViewModel {
            lines: lines.to_vec(),
            thinking,
        };

        let mut composer_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if let Some(chips) =
            composer_add::render_composer_chips(self.font, plan_mode, goal_mode, &attachments)
        {
            composer_col.add_child(chips);
        }
        composer_col.add_child(
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
            .with_min_width(0.0)
            .with_height(input_height)
            .with_max_height(multiline_input::box_height(
                &"x".repeat(multiline_input::DEFAULT_COLS * multiline_input::MAX_LINES),
                multiline_input::DEFAULT_COLS,
            ))
            .finish(),
        );
        composer_col.add_child(self.composer_bar(
            busy,
            draft.trim().is_empty(),
            access_mode,
            model_rate,
        ));

        let composer_inner = Container::new(composer_col.finish())
            .with_background(theme::panel())
            .with_border(Border::all(1.0).with_border_color(input_border))
            .with_corner_radius(warpui::elements::CornerRadius::with_all(
                warpui::elements::Radius::Pixels(16.0),
            ))
            .finish();

        let composer_card = ConstrainedBox::new(composer_inner)
            .with_max_width(AGENT_THREAD_MAX_WIDTH)
            .with_width(AGENT_THREAD_MAX_WIDTH)
            .finish();

        let idle_composer = Self::is_idle_composer_state(&active_sidebar_session_id);
        let composer_folder = if idle_composer {
            None
        } else {
            sidebar_sessions
                .iter()
                .find(|s| s.id == active_sidebar_session_id)
                .and_then(|s| s.project_id.as_ref())
                .and_then(|pid| projects.iter().find(|p| p.id == *pid))
                .map(|p| {
                    let name = p
                        .folder_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or(p.label.as_str())
                        .to_string();
                    let path = p.folder_path.display().to_string();
                    (name, path)
                })
        };
        let composer_surface = composer_card;

        let mut main_stack = Stack::new();

        if idle_composer {
            let idle_body = Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(self.agent_header(&thread_title))
                .with_child(
                    Expanded::new(1.0, self.composer_center_layer(composer_surface)).finish(),
                )
                .finish();
            main_stack.add_child(idle_body);
            if access_menu_open || model_menu_open || add_menu_open {
                let chips_extra = if plan_mode || goal_mode || !attachments.is_empty() {
                    40.0
                } else {
                    0.0
                };
                let card_height = input_height + COMPOSER_CHROME_HEIGHT + chips_extra;
                main_stack.add_child(self.render_idle_composer_popovers(
                    card_height,
                    access_mode,
                    model_rate,
                    access_menu_open,
                    model_menu_open,
                    add_menu_open,
                    goal_mode,
                    plan_mode,
                ));
            }
        } else {
            let thread_body = Container::new(
                Align::new(render_transcript(&transcript_model, self.font, self.mono)).finish(),
            )
            .with_padding_left(24.0)
            .with_padding_right(24.0)
            .with_padding_top(28.0)
            .with_padding_bottom(AGENT_THREAD_BOTTOM_PAD)
            .with_background(theme::canvas())
            .finish();

            let thread_scroll = ClippedScrollable::vertical(
                self.thread_scroll.clone(),
                thread_body,
                ScrollbarWidth::Auto,
                Fill::None,
                Fill::None,
                Fill::None,
            )
            .finish();

            let thread_column = Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(self.agent_header(&thread_title))
                .with_child(Expanded::new(1.0, thread_scroll).finish())
                .finish();

            main_stack.add_child(thread_column);
            main_stack.add_child(self.composer_bottom_layer(composer_folder.clone(), composer_surface));
            if access_menu_open || model_menu_open || add_menu_open {
                main_stack.add_child(self.render_composer_popovers(
                    access_mode,
                    model_rate,
                    access_menu_open,
                    model_menu_open,
                    add_menu_open,
                    goal_mode,
                    plan_mode,
                    composer_folder.is_some(),
                ));
            }
        }

        // No dim scrim for access/model popovers (HTML has none; scrim blocked clicks).
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
                    &expanded_project_ids,
                    &active_project_id,
                    &active_sidebar_session_id,
                    &sidebar_search,
                    sidebar_search_focused,
                    &archived_ids,
                    chats_menu_open,
                    project_row_menu.as_deref(),
                    sidebar_hover.as_deref(),
                    project_head_hover.as_deref(),
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

        // Project-row menu is in-tree (no scrim) so the dropdown stays clickable.
        // Files/media modals bring their own scrim.
        let overlay_open = chats_menu_open
            || session_context_menu.is_some()
            || project_create.is_some()
            || project_delete.is_some();

        let mut root_stack = Stack::new();
        root_stack.add_child(panel);
        if overlay_open {
            root_stack.add_child(self.overlay_scrim());
        }
        if let Some(create) = &project_create {
            root_stack.add_child(project_create_modal::render_project_create_modal(
                self.font, create,
            ));
        }
        if let Some(delete) = &project_delete {
            let (label, path) = projects
                .iter()
                .find(|p| p.id == delete.project_id)
                .map(|p| {
                    (
                        p.label.clone(),
                        p.folder_path.display().to_string(),
                    )
                })
                .unwrap_or_else(|| ("—".into(), String::new()));
            root_stack.add_child(project_delete_modal::render_project_delete_modal(
                self.font,
                self.mono,
                delete,
                &label,
                &path,
            ));
        }
        if let Some(files) = &files_modal {
            root_stack.add_child(composer_add::render_files_modal(
                self.font, self.mono, files,
            ));
        }
        if let Some(media) = &media_modal {
            root_stack.add_child(composer_add::render_media_modal(self.font, media));
        }
        if let Some((session_id, archived, x, y)) = session_context_menu {
            root_stack.add_child(sidebar::render_session_context_menu(
                self.font,
                &session_id,
                archived,
                x,
                y,
            ));
        }

        tab_content_fill(
            EventHandler::new(root_stack.finish())
                .with_always_handle()
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(AgentPanelAction::DismissSidebarMenus);
                    ctx.dispatch_typed_action(AgentPanelAction::CloseSessionContextMenu);
                    ctx.dispatch_typed_action(AgentPanelAction::DismissComposerMenus);
                    DispatchEventResult::PropagateToParent
                })
                .on_keydown({
                    let state = Arc::clone(&self.state);
                    let notify_tx = self.generation_notify_tx.clone();
                    move |ctx, _, keystroke| {
                        if keystroke.key == "escape" {
                            ctx.dispatch_typed_action(AgentPanelAction::CloseFilesModal);
                            ctx.dispatch_typed_action(AgentPanelAction::CloseMediaModal);
                            ctx.dispatch_typed_action(AgentPanelAction::CloseProjectDeleteModal);
                            ctx.dispatch_typed_action(AgentPanelAction::CloseProjectCreateModal);
                            ctx.dispatch_typed_action(AgentPanelAction::CloseSessionContextMenu);
                            ctx.dispatch_typed_action(AgentPanelAction::DismissSidebarMenus);
                            ctx.dispatch_typed_action(AgentPanelAction::DismissComposerMenus);
                            return DispatchEventResult::StopPropagation;
                        }
                        if let Some(action) =
                            Self::keystroke_action(&state, keystroke)
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
            format!("Warp AI，Codex，{mode} 模式"),
            "上下方向键切换对话或全自动模式。Tab 聚焦输入框，Enter 发送。",
            WarpA11yRole::WindowRole,
        ))
    }

    fn accessibility_data(&self, _ctx: &mut ViewContext<Self>) -> Option<AccessibilityData> {
        Some(AccessibilityData {
            content: "Warp AI 面板".into(),
        })
    }
}

impl TypedActionView for AgentPanelView {
    type Action = AgentPanelAction;

    fn handle_action(&mut self, action: &AgentPanelAction, ctx: &mut ViewContext<Self>) {
        match action {
            AgentPanelAction::SetVisible(visible) => self.set_tab_visible(*visible, ctx),
            AgentPanelAction::SelectAccessMode(mode) => self.select_access_mode(*mode, ctx),
            AgentPanelAction::ToggleAccessMenu => self.toggle_access_menu(ctx),
            AgentPanelAction::SelectModelRate(rate) => self.select_model_rate(*rate, ctx),
            AgentPanelAction::ToggleModelMenu => self.toggle_model_menu(ctx),
            AgentPanelAction::ToggleAddMenu => self.toggle_add_menu(ctx),
            AgentPanelAction::OpenFilesModal => self.open_files_modal(ctx),
            AgentPanelAction::CloseFilesModal => self.close_files_modal(ctx),
            AgentPanelAction::SelectDemoFile(i) => self.select_demo_file(*i, ctx),
            AgentPanelAction::ConfirmFilesModal => self.confirm_files_modal(ctx),
            AgentPanelAction::OpenMediaModal => self.open_media_modal(ctx),
            AgentPanelAction::CloseMediaModal => self.close_media_modal(ctx),
            AgentPanelAction::SelectDemoMedia(i) => self.select_demo_media(*i, ctx),
            AgentPanelAction::ConfirmMediaModal => self.confirm_media_modal(ctx),
            AgentPanelAction::ToggleGoalMode => self.toggle_goal_mode(ctx),
            AgentPanelAction::TogglePlanMode => self.toggle_plan_mode(ctx),
            AgentPanelAction::ClearGoalMode => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.goal_mode = false;
                }
                ctx.notify();
            }
            AgentPanelAction::ClearPlanMode => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.plan_mode = false;
                }
                ctx.notify();
            }
            AgentPanelAction::RemoveAttachment(i) => {
                if let Ok(mut panel) = self.state.lock() {
                    if *i < panel.attachments.len() {
                        panel.attachments.remove(*i);
                    }
                }
                ctx.notify();
            }
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
            AgentPanelAction::NewStandaloneChat => self.new_standalone_chat(ctx),
            AgentPanelAction::NewProjectThread(id) => self.new_project_thread(id.clone(), ctx),
            AgentPanelAction::ToggleProjectExpanded(id) => {
                self.toggle_project_expanded(id.clone(), ctx)
            }
            AgentPanelAction::OpenProjectCreateModal => self.open_project_create_modal(ctx),
            AgentPanelAction::CloseProjectCreateModal => self.close_project_create_modal(ctx),
            AgentPanelAction::ProjectCreateNext => self.project_create_next(ctx),
            AgentPanelAction::ProjectCreateUseExistingFolder => {
                self.project_create_use_existing_folder(ctx)
            }
            AgentPanelAction::ProjectCreateSubmit => self.project_create_submit(ctx),
            AgentPanelAction::FocusProjectCreateName => {
                if let Ok(mut panel) = self.state.lock() {
                    if let Some(create) = panel.project_create.as_mut() {
                        create.name_focused = true;
                    }
                }
                ctx.notify();
            }
            AgentPanelAction::ProjectCreateNameEdit(edit) => {
                if let Ok(mut panel) = self.state.lock() {
                    if let Some(create) = panel.project_create.as_mut() {
                        let mut field = TextFieldState::new();
                        field.apply(&mut create.name, edit);
                        create.name_invalid = false;
                        create.name_focused = true;
                    }
                }
                ctx.notify();
            }
            AgentPanelAction::OpenProjectRowMenu(id) => {
                self.open_project_row_menu(id.clone(), ctx)
            }
            AgentPanelAction::OpenProjectDeleteModal(id) => {
                self.open_project_delete_modal(id.clone(), ctx)
            }
            AgentPanelAction::CloseProjectDeleteModal => self.close_project_delete_modal(ctx),
            AgentPanelAction::SetProjectDeleteMode(delete_local) => {
                self.set_project_delete_mode(*delete_local, ctx)
            }
            AgentPanelAction::ConfirmDeleteProject => self.confirm_delete_project(ctx),
            AgentPanelAction::RenameProject(id) => self.rename_project(id.clone(), ctx),
            AgentPanelAction::DeleteActiveStandaloneChat => {
                self.delete_active_standalone_chat(ctx)
            }
            AgentPanelAction::FocusSidebarSearch => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.sidebar_search_focused = true;
                    panel.input_focused = false;
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            AgentPanelAction::ToggleChatsMenu => self.toggle_chats_menu(ctx),
            AgentPanelAction::DismissSidebarMenus => self.dismiss_sidebar_menus(ctx),
            AgentPanelAction::DeleteActiveProject => {
                let id = self
                    .state
                    .lock()
                    .map(|p| p.active_project_id.clone())
                    .unwrap_or_default();
                if !id.is_empty() {
                    self.open_project_delete_modal(id, ctx);
                }
            }
            AgentPanelAction::ArchiveSession(id) => self.archive_session(id.clone(), ctx),
            AgentPanelAction::ArchiveActiveSession => {
                let id = self
                    .state
                    .lock()
                    .map(|p| p.active_sidebar_session_id.clone())
                    .unwrap_or_default();
                if !id.is_empty() {
                    self.archive_session(id, ctx);
                }
            }
            AgentPanelAction::RestoreSession(id) => self.restore_session(id.clone(), ctx),
            AgentPanelAction::DeleteSession(id) => self.delete_session(id.clone(), ctx),
            AgentPanelAction::OpenSessionContextMenu { id, archived, x, y } => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.project_row_menu = None;
                    panel.chats_menu_open = false;
                    panel.session_context_menu = Some((id.clone(), *archived, *x, *y));
                }
                ctx.notify();
            }
            AgentPanelAction::CloseSessionContextMenu => self.close_session_context_menu(ctx),
            AgentPanelAction::CopyUserPrompt => self.copy_user_prompt(ctx),
            AgentPanelAction::ClearAndUnfocus => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.draft.clear();
                    panel.field_state.clear_marked();
                    panel.input_focused = false;
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            AgentPanelAction::ToggleComposerFocus => {
                if let Ok(mut panel) = self.state.lock() {
                    if panel.sidebar_search_focused {
                        panel.sidebar_search_focused = false;
                        panel.input_focused = true;
                    } else {
                        panel.input_focused = !panel.input_focused;
                        panel.sidebar_search_focused = false;
                    }
                }
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            AgentPanelAction::NewConversation => self.new_conversation(ctx),
            AgentPanelAction::LaunchTerminal => self.launch_terminal(ctx),
            AgentPanelAction::RefreshStatus => self.refresh_status(ctx),
            AgentPanelAction::Stop => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.busy = false;
                    panel.polling_session = false;
                    if !panel.active_sidebar_session_id.is_empty() {
                        let id = panel.active_sidebar_session_id.clone();
                        if let Some(session) =
                            panel.sidebar_sessions.iter_mut().find(|s| s.id == id)
                        {
                            session.running = false;
                        }
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
            AgentPanelAction::SetSidebarHover(key) => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.sidebar_hover = key.clone();
                }
                ctx.notify();
            }
            AgentPanelAction::ClearSidebarHoverIf(key) => {
                if let Ok(mut panel) = self.state.lock() {
                    if panel.sidebar_hover.as_deref() == Some(key.as_str()) {
                        panel.sidebar_hover = None;
                    }
                }
                ctx.notify();
            }
            AgentPanelAction::SetProjectHeadHover(id) => {
                if let Ok(mut panel) = self.state.lock() {
                    panel.project_head_hover = id.clone();
                }
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
            AgentPanelAction::SelectAccessMode(AgentAccessMode::FullAccess) => {
                AccessibilityContent::new_without_help("完全访问", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::SelectAccessMode(AgentAccessMode::WorkspaceWrite) => {
                AccessibilityContent::new_without_help("工作区写入", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::ToggleAccessMenu => {
                AccessibilityContent::new_without_help("访问权限菜单", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::ToggleAddMenu
            | AgentPanelAction::OpenFilesModal
            | AgentPanelAction::CloseFilesModal
            | AgentPanelAction::SelectDemoFile(_)
            | AgentPanelAction::ConfirmFilesModal
            | AgentPanelAction::OpenMediaModal
            | AgentPanelAction::CloseMediaModal
            | AgentPanelAction::SelectDemoMedia(_)
            | AgentPanelAction::ConfirmMediaModal
            | AgentPanelAction::ToggleGoalMode
            | AgentPanelAction::TogglePlanMode
            | AgentPanelAction::ClearGoalMode
            | AgentPanelAction::ClearPlanMode
            | AgentPanelAction::RemoveAttachment(_) => {
                AccessibilityContent::new_without_help("添加", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::SelectModelRate(rate) => AccessibilityContent::new_without_help(
                rate.chip_label(),
                WarpA11yRole::MenuItemRole,
            ),
            AgentPanelAction::ToggleModelMenu => {
                AccessibilityContent::new_without_help("模型倍率菜单", WarpA11yRole::ButtonRole)
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
            AgentPanelAction::NewProject
            | AgentPanelAction::NewThread
            | AgentPanelAction::NewStandaloneChat
            | AgentPanelAction::NewProjectThread(_)
            | AgentPanelAction::OpenProjectCreateModal
            | AgentPanelAction::ProjectCreateSubmit => {
                AccessibilityContent::new_without_help("新建侧栏项", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::ToggleProjectExpanded(_)
            | AgentPanelAction::RenameProject(_)
            | AgentPanelAction::OpenProjectRowMenu(_) => {
                AccessibilityContent::new_without_help("项目操作", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::CloseProjectCreateModal
            | AgentPanelAction::ProjectCreateNext
            | AgentPanelAction::ProjectCreateUseExistingFolder
            | AgentPanelAction::FocusProjectCreateName
            | AgentPanelAction::ProjectCreateNameEdit(_) => {
                AccessibilityContent::new_without_help("创建项目", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::OpenProjectDeleteModal(_)
            | AgentPanelAction::CloseProjectDeleteModal
            | AgentPanelAction::SetProjectDeleteMode(_)
            | AgentPanelAction::ConfirmDeleteProject
            | AgentPanelAction::DeleteActiveProject => {
                AccessibilityContent::new_without_help("删除项目", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::DeleteActiveStandaloneChat => {
                AccessibilityContent::new_without_help("删除独立对话", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::FocusSidebarSearch => {
                AccessibilityContent::new_without_help("聚焦会话搜索", WarpA11yRole::TextfieldRole)
            }
            AgentPanelAction::ToggleChatsMenu => {
                AccessibilityContent::new_without_help("侧栏菜单", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::DismissSidebarMenus | AgentPanelAction::CloseSessionContextMenu => {
                return ActionAccessibilityContent::Empty;
            }
            AgentPanelAction::ArchiveSession(_)
            | AgentPanelAction::ArchiveActiveSession
            | AgentPanelAction::RestoreSession(_)
            | AgentPanelAction::DeleteSession(_) => {
                AccessibilityContent::new_without_help("侧栏操作", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::OpenSessionContextMenu { .. } => {
                AccessibilityContent::new_without_help("会话上下文菜单", WarpA11yRole::MenuItemRole)
            }
            AgentPanelAction::CopyUserPrompt => {
                AccessibilityContent::new_without_help("复制指令", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::ClearAndUnfocus | AgentPanelAction::ToggleComposerFocus => {
                return ActionAccessibilityContent::Empty;
            }
            AgentPanelAction::Stop => {
                AccessibilityContent::new_without_help("停止 Agent", WarpA11yRole::ButtonRole)
            }
            AgentPanelAction::TextFieldEdit(_) => AccessibilityContent::new_without_help(
                "编辑 Agent 输入",
                WarpA11yRole::TextfieldRole,
            ),
            AgentPanelAction::SetVisible(_) | AgentPanelAction::RefreshStatus => {
                return ActionAccessibilityContent::Empty;
            }
            AgentPanelAction::SetSidebarHover(_)
            | AgentPanelAction::ClearSidebarHoverIf(_)
            | AgentPanelAction::SetProjectHeadHover(_) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    fn sample_panel(projects: Vec<sidebar::AgentProject>, active_project_id: &str) -> PanelState {
        PanelState {
            status: String::new(),
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
            projects,
            sidebar_sessions: Vec::new(),
            active_project_id: active_project_id.to_string(),
            active_sidebar_session_id: String::new(),
            thread_title: String::new(),
            sidebar_search: String::new(),
            sidebar_search_focused: false,
            archived_ids: HashSet::new(),
            expanded_project_ids: HashSet::new(),
            project_create: None,
            project_row_menu: None,
            project_delete: None,
            chats_menu_open: false,
            session_context_menu: None,
            access_menu_open: false,
            model_menu_open: false,
            add_menu_open: false,
            plan_mode: false,
            goal_mode: false,
            attachments: Vec::new(),
            files_modal: None,
            media_modal: None,
            field_state: TextFieldState::new(),
            sidebar_hover: None,
            project_head_hover: None,
        }
    }

    fn sample_project(id: &str, path: &str) -> sidebar::AgentProject {
        sidebar::AgentProject {
            id: id.into(),
            label: "项目".into(),
            time: "刚刚".into(),
            folder_path: PathBuf::from(path),
        }
    }

    #[test]
    fn is_idle_composer_state_when_no_session_selected() {
        assert!(AgentPanelView::is_idle_composer_state(""));
        assert!(!AgentPanelView::is_idle_composer_state("session-1"));
    }

    #[test]
    fn ensure_sidebar_session_for_send_creates_standalone_chat() {
        let mut panel = sample_panel(vec![], "");
        panel.draft = "hello".into();
        AgentPanelView::ensure_sidebar_session_for_send(&mut panel);
        assert_eq!(panel.sidebar_sessions.len(), 1);
        assert!(panel.sidebar_sessions[0].project_id.is_none());
        assert_eq!(panel.draft, "hello");
        assert!(!panel.active_sidebar_session_id.is_empty());
    }

    #[test]
    fn ensure_sidebar_session_for_send_attaches_active_project() {
        let mut panel = sample_panel(vec![sample_project("p1", "D:\\p1")], "p1");
        panel.draft = "task".into();
        AgentPanelView::ensure_sidebar_session_for_send(&mut panel);
        assert_eq!(panel.sidebar_sessions.len(), 1);
        assert_eq!(panel.sidebar_sessions[0].project_id.as_deref(), Some("p1"));
        assert_eq!(panel.draft, "task");
    }

    #[test]
    fn ensure_sidebar_session_for_send_is_noop_when_session_active() {
        let mut panel = sample_panel(vec![], "");
        panel.active_sidebar_session_id = "session-existing".into();
        panel.draft = "keep".into();
        AgentPanelView::ensure_sidebar_session_for_send(&mut panel);
        assert!(panel.sidebar_sessions.is_empty());
        assert_eq!(panel.draft, "keep");
        assert_eq!(panel.active_sidebar_session_id, "session-existing");
    }

    #[test]
    fn new_standalone_chat_does_not_create_project() {
        let mut panel = sample_panel(vec![], "");
        AgentPanelView::insert_new_session(&mut panel, None);
        assert!(panel.projects.is_empty());
        assert_eq!(panel.sidebar_sessions.len(), 1);
        assert!(panel.sidebar_sessions[0].project_id.is_none());
    }

    #[test]
    fn new_project_thread_attaches_to_project() {
        let mut panel = sample_panel(vec![sample_project("p1", "D:\\p1")], "p1");
        AgentPanelView::insert_new_session(&mut panel, Some("p1".into()));
        assert_eq!(panel.sidebar_sessions.len(), 1);
        assert_eq!(panel.sidebar_sessions[0].project_id.as_deref(), Some("p1"));
    }

    #[tokio::test]
    async fn resolve_session_cwd_uses_project_folder() {
        let dir = std::env::temp_dir().join(format!("wormhole-agent-cwd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut panel = sample_panel(vec![sample_project("p1", dir.to_str().unwrap())], "p1");
        let session_id: String = "session-1".into();
        panel.sidebar_sessions.push(sidebar::AgentSession {
            id: session_id.clone(),
            label: "t".into(),
            project_id: Some("p1".into()),
            time: "刚刚".into(),
            running: false,
            prompt: String::new(),
        });
        panel.active_sidebar_session_id = session_id;
        let cwd = AgentPanelView::resolve_session_cwd(&panel, Path::new("."))
            .await
            .expect("cwd");
        assert_eq!(cwd, Some(dir.display().to_string()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn is_local_sidebar_session_matches_ephemeral_ids() {
        assert!(AgentPanelView::is_local_sidebar_session("session-1710000000"));
        assert!(!AgentPanelView::is_local_sidebar_session("codex-abc123"));
    }

    #[test]
    fn composer_send_on_enter_requires_non_empty_draft() {
        assert!(AgentPanelView::composer_send_on_enter(false, false, "hello"));
        assert!(!AgentPanelView::composer_send_on_enter(false, false, "   "));
        assert!(!AgentPanelView::composer_send_on_enter(true, false, "hello"));
        assert!(!AgentPanelView::composer_send_on_enter(false, true, "hello"));
    }

    #[test]
    fn select_session_prep_clears_busy_for_local_session() {
        let mut panel = sample_panel(vec![sample_project("p1", "D:\\p1")], "p1");
        panel.busy = true;
        panel.polling_session = true;
        panel.sidebar_sessions.push(sidebar::AgentSession {
            id: "session-local".into(),
            label: "本地".into(),
            project_id: Some("p1".into()),
            time: "刚刚".into(),
            running: false,
            prompt: String::new(),
        });
        assert!(AgentPanelView::is_local_sidebar_session("session-local"));
        panel.busy = false;
        panel.polling_session = false;
        panel.active_sidebar_session_id = "session-local".into();
        assert!(!panel.busy);
        assert!(!panel.polling_session);
    }

    #[test]
    fn codex_chat_completion_skips_duplicate_assistant_line() {
        let mut panel = sample_panel(vec![], "");
        AgentPanelView::push_line(
            &mut panel,
            TranscriptLine {
                channel: "assistant".into(),
                text: "已打开 Chrome 并搜索了鸡哥。".into(),
                level: "info".into(),
            },
        );
        AgentPanelView::push_line(
            &mut panel,
            TranscriptLine {
                channel: "status".into(),
                text: "turn completed".into(),
                level: "info".into(),
            },
        );
        let before = panel.lines.len();
        AgentPanelView::append_chat_completion_transcript(
            &mut panel,
            AgentTurnBackend::Codex,
            "已打开 Chrome 并搜索了鸡哥。",
        );
        assert_eq!(panel.lines.len(), before);
    }

    #[test]
    fn cursor_fallback_completion_appends_assistant_line() {
        let mut panel = sample_panel(vec![], "");
        AgentPanelView::append_chat_completion_transcript(
            &mut panel,
            AgentTurnBackend::CursorFallback,
            "任务完成。",
        );
        assert_eq!(panel.lines.len(), 1);
        assert_eq!(panel.lines[0].channel, "assistant");
        assert_eq!(panel.lines[0].text, "任务完成。");
    }

    #[test]
    fn composer_model_chip_label_includes_rate() {
        assert_eq!(
            composer_model_chip_label(AgentModelRate::X03),
            "GPT-5.5 ×0.3"
        );
        assert_eq!(
            composer_model_chip_label(AgentModelRate::X01),
            "GPT-5.5 ×0.1"
        );
    }
}
