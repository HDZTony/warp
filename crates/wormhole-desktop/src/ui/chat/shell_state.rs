use std::sync::{Arc, Mutex};

use crate::ui::panel_primitives::StatusTone;

#[derive(Debug, Clone)]
pub struct PendingOutgoingAttachment {
    pub kind: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct PendingOutgoingMessage {
    pub client_id: String,
    pub conv_id: String,
    pub body: String,
    pub sent_at: u64,
    pub attachments: Vec<PendingOutgoingAttachment>,
}

/// Optimistic header/thread state while `chat_start_conversation` is in flight.
#[derive(Debug, Clone)]
pub struct PendingOpenChat {
    pub title: String,
    pub os: String,
    pub presence: String,
}

#[derive(Debug, Clone)]
pub struct ChatShellState {
    pub thread_search_open: bool,
    pub profile_open: bool,
    pub header_menu_open: bool,
    pub mute_flyout_open: bool,
    pub sidebar_menu_open: bool,
    pub contacts_open: bool,
    pub contacts_add_open: bool,
    pub channel_open: bool,
    pub calls_open: bool,
    /// `overview` | `picker` | `settings` | `privacy` | `confirm`
    pub calls_subview: String,
    pub calls_menu_open: bool,
    pub favorites_only: bool,
    pub thread_search_query: String,
    pub thread_search_from_ms: Option<u64>,
    pub thread_search_to_ms: Option<u64>,
    pub thread_search_request_tick: u64,
    pub thread_search_nav_tick: u64,
    pub thread_search_nav_delta: i8,
    pub thread_search_current: usize,
    pub thread_search_total: usize,
    pub thread_search_loading: bool,
    pub thread_search_error: Option<String>,
    pub toast: String,
    pub toast_tone: StatusTone,
    pub message_tick: u64,
    /// Bumped when sidebar selection changes so header refreshes immediately.
    pub selection_tick: u64,
    /// Bumped when chat UI prefs (mute/hide/wallpaper) change so sidebar/thread rebuild.
    pub prefs_tick: u64,
    /// Bumped when per-conversation wallpaper changes.
    pub wallpaper_tick: u64,
    /// `idle` | `ringing` | `incoming` | `active`
    pub voice_call_phase: String,
    pub voice_call_active: bool,
    /// Peer node opened in the live viewer for the active voice call.
    pub voice_live_peer: Option<String>,
    pub pending_open: Option<PendingOpenChat>,
    pub selected_summary: Option<PendingOpenChat>,
    /// Last failure from opening a placeholder conversation (shown under compose).
    pub open_error: Option<String>,
    pub pending_outgoing: Vec<PendingOutgoingMessage>,
}

impl Default for ChatShellState {
    fn default() -> Self {
        Self {
            thread_search_open: false,
            profile_open: false,
            header_menu_open: false,
            mute_flyout_open: false,
            sidebar_menu_open: false,
            contacts_open: false,
            contacts_add_open: false,
            channel_open: false,
            calls_open: false,
            calls_subview: "overview".into(),
            calls_menu_open: false,
            favorites_only: false,
            thread_search_query: String::new(),
            thread_search_from_ms: None,
            thread_search_to_ms: None,
            thread_search_request_tick: 0,
            thread_search_nav_tick: 0,
            thread_search_nav_delta: 0,
            thread_search_current: 0,
            thread_search_total: 0,
            thread_search_loading: false,
            thread_search_error: None,
            toast: String::new(),
            toast_tone: StatusTone::Neutral,
            message_tick: 0,
            selection_tick: 0,
            prefs_tick: 0,
            wallpaper_tick: 0,
            voice_call_phase: "idle".into(),
            voice_call_active: false,
            voice_live_peer: None,
            pending_open: None,
            selected_summary: None,
            open_error: None,
            pending_outgoing: Vec::new(),
        }
    }
}

pub type SharedChatShellState = Arc<Mutex<ChatShellState>>;

pub fn new_shared_shell_state() -> SharedChatShellState {
    Arc::new(Mutex::new(ChatShellState::default()))
}

impl ChatShellState {
    pub fn show_toast(&mut self, text: impl Into<String>, tone: StatusTone) {
        self.toast = text.into();
        self.toast_tone = tone;
    }

    pub fn clear_toast(&mut self) {
        self.toast.clear();
        self.toast_tone = StatusTone::Neutral;
    }

    pub fn close_overlays(&mut self) {
        self.header_menu_open = false;
        self.mute_flyout_open = false;
        self.sidebar_menu_open = false;
        self.calls_menu_open = false;
    }

    pub fn close_contacts(&mut self) {
        self.contacts_open = false;
        self.contacts_add_open = false;
    }

    pub fn close_channel(&mut self) {
        self.channel_open = false;
    }

    pub fn close_calls(&mut self) {
        self.calls_open = false;
        self.calls_menu_open = false;
        self.calls_subview = "overview".into();
    }

    pub fn open_contacts(&mut self) {
        self.close_overlays();
        self.close_calls();
        self.close_channel();
        self.contacts_open = true;
        self.contacts_add_open = false;
    }

    pub fn open_channel(&mut self) {
        self.close_overlays();
        self.close_contacts();
        self.close_calls();
        self.channel_open = true;
    }

    pub fn open_calls(&mut self) {
        self.close_overlays();
        self.close_contacts();
        self.close_channel();
        self.calls_open = true;
        self.calls_subview = "overview".into();
        self.calls_menu_open = false;
    }

    pub fn close_thread_search(&mut self) {
        self.thread_search_open = false;
        self.thread_search_query.clear();
        self.thread_search_from_ms = None;
        self.thread_search_to_ms = None;
        self.thread_search_current = 0;
        self.thread_search_total = 0;
        self.thread_search_loading = false;
        self.thread_search_error = None;
        self.thread_search_request_tick = self.thread_search_request_tick.saturating_add(1);
    }

    pub fn request_thread_search(&mut self) {
        self.thread_search_request_tick = self.thread_search_request_tick.saturating_add(1);
        self.thread_search_current = 0;
        self.thread_search_total = 0;
        self.thread_search_error = None;
    }

    pub fn navigate_thread_search(&mut self, delta: i8) {
        self.thread_search_nav_delta = delta.signum();
        self.thread_search_nav_tick = self.thread_search_nav_tick.saturating_add(1);
    }

    pub fn push_pending_outgoing(
        &mut self,
        client_id: String,
        conv_id: String,
        body: String,
        sent_at: u64,
        attachments: Vec<PendingOutgoingAttachment>,
    ) {
        self.pending_outgoing.push(PendingOutgoingMessage {
            client_id,
            conv_id,
            body,
            sent_at,
            attachments,
        });
    }

    pub fn remove_pending(&mut self, client_id: &str) {
        self.pending_outgoing
            .retain(|pending| pending.client_id != client_id);
    }

    pub fn pending_for_conv(&self, conv_id: &str) -> Vec<PendingOutgoingMessage> {
        self.pending_outgoing
            .iter()
            .filter(|pending| pending.conv_id == conv_id)
            .cloned()
            .collect()
    }

    /// Bumps the thread refresh counter so [`super::thread::ChatThreadView`] refetches messages.
    pub fn bump_message_tick(&mut self) {
        self.message_tick = self.message_tick.saturating_add(1);
    }

    pub fn bump_selection_tick(&mut self) {
        self.selection_tick = self.selection_tick.saturating_add(1);
    }

    pub fn bump_wallpaper_tick(&mut self) {
        self.wallpaper_tick = self.wallpaper_tick.saturating_add(1);
    }

    pub fn bump_prefs_tick(&mut self) {
        self.prefs_tick = self.prefs_tick.saturating_add(1);
    }

    pub fn set_voice_call_phase(&mut self, phase: impl Into<String>) {
        let phase = phase.into();
        self.voice_call_active = phase == "active";
        self.voice_call_phase = phase;
    }

    pub fn set_pending_open(&mut self, title: String, os: String, presence: String) {
        self.open_error = None;
        self.pending_open = Some(PendingOpenChat {
            title,
            os,
            presence,
        });
        self.bump_selection_tick();
    }

    pub fn clear_pending_open(&mut self) {
        if self.pending_open.take().is_some() {
            self.bump_selection_tick();
        }
    }

    pub fn set_selected_summary(&mut self, title: String, os: String, presence: String) {
        self.selected_summary = Some(PendingOpenChat {
            title,
            os,
            presence,
        });
    }

    pub fn clear_selected_summary(&mut self) {
        self.selected_summary = None;
    }

    pub fn set_open_error(&mut self, err: impl Into<String>) {
        self.open_error = Some(err.into());
        self.bump_selection_tick();
    }

    pub fn clear_open_error(&mut self) {
        if self.open_error.take().is_some() {
            self.bump_selection_tick();
        }
    }
}

/// Whether a desktop `chat-event` payload should refresh the open thread and sidebar.
pub fn chat_event_triggers_refresh(kind: &str) -> bool {
    matches!(
        kind,
        "message_received" | "conversation_added" | "sync_required"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_message_tick_increments() {
        let mut state = ChatShellState::default();
        assert_eq!(state.message_tick, 0);
        state.bump_message_tick();
        assert_eq!(state.message_tick, 1);
        state.bump_message_tick();
        assert_eq!(state.message_tick, 2);
    }

    #[test]
    fn chat_event_triggers_refresh_for_known_kinds() {
        assert!(chat_event_triggers_refresh("message_received"));
        assert!(chat_event_triggers_refresh("conversation_added"));
        assert!(chat_event_triggers_refresh("sync_required"));
        assert!(!chat_event_triggers_refresh("typing_changed"));
        assert!(!chat_event_triggers_refresh(""));
    }

    #[test]
    fn open_error_clears_on_pending_open() {
        let mut state = ChatShellState::default();
        state.set_open_error("无法开始会话: offline");
        assert!(state.open_error.is_some());
        state.set_pending_open("PC · host".into(), "Windows".into(), "offline".into());
        assert!(state.open_error.is_none());
        assert!(state.pending_open.is_some());
    }

    #[test]
    fn search_request_and_navigation_are_explicit() {
        let mut state = ChatShellState::default();
        state.thread_search_current = 4;
        state.thread_search_total = 9;
        state.request_thread_search();
        assert_eq!(state.thread_search_request_tick, 1);
        assert_eq!(state.thread_search_current, 0);
        assert_eq!(state.thread_search_total, 0);
        state.navigate_thread_search(8);
        assert_eq!(state.thread_search_nav_tick, 1);
        assert_eq!(state.thread_search_nav_delta, 1);
    }
}
