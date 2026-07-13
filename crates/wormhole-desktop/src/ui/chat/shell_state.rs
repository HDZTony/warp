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
    pub online: bool,
}

#[derive(Debug, Clone)]
pub struct ChatShellState {
    pub thread_search_open: bool,
    pub profile_open: bool,
    pub header_menu_open: bool,
    pub mute_flyout_open: bool,
    pub remote_desktop_active: bool,
    pub thread_search_query: String,
    pub toast: String,
    pub toast_tone: StatusTone,
    pub message_tick: u64,
    /// Bumped when sidebar selection changes so header refreshes immediately.
    pub selection_tick: u64,
    /// Bumped when chat UI prefs (mute/hide) change so sidebar rebuilds.
    pub prefs_tick: u64,
    pub pending_open: Option<PendingOpenChat>,
    pub pending_outgoing: Vec<PendingOutgoingMessage>,
}

impl Default for ChatShellState {
    fn default() -> Self {
        Self {
            thread_search_open: false,
            profile_open: false,
            header_menu_open: false,
            mute_flyout_open: false,
            remote_desktop_active: false,
            thread_search_query: String::new(),
            toast: String::new(),
            toast_tone: StatusTone::Neutral,
            message_tick: 0,
            selection_tick: 0,
            prefs_tick: 0,
            pending_open: None,
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
    }

    pub fn close_thread_search(&mut self) {
        self.thread_search_open = false;
        self.thread_search_query.clear();
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

    pub fn bump_prefs_tick(&mut self) {
        self.prefs_tick = self.prefs_tick.saturating_add(1);
    }

    pub fn set_pending_open(&mut self, title: String, os: String, online: bool) {
        self.pending_open = Some(PendingOpenChat { title, os, online });
        self.bump_selection_tick();
    }

    pub fn clear_pending_open(&mut self) {
        if self.pending_open.take().is_some() {
            self.bump_selection_tick();
        }
    }
}

/// Whether a desktop `chat-event` payload should refresh the open thread and sidebar.
pub fn chat_event_triggers_refresh(kind: &str) -> bool {
    matches!(kind, "message_received" | "conversation_added")
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
        assert!(!chat_event_triggers_refresh("typing_changed"));
        assert!(!chat_event_triggers_refresh(""));
    }
}
