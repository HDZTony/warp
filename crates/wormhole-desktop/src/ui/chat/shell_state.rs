use std::sync::{Arc, Mutex};

use crate::ui::panel_primitives::StatusTone;

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
}
