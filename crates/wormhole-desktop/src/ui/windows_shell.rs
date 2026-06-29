use std::sync::{Arc, Mutex};

use warpui::platform::TerminationMode;
use warpui::{AppContext, View, ViewContext, WindowId};

use crate::coordinator::CoordinatorState;

pub fn hide_main_window(window_id: WindowId, ctx: &mut AppContext) {
    ctx.windows().hide_window(window_id);
}

pub fn show_main_window(window_id: WindowId, ctx: &mut AppContext) {
    ctx.windows().show_window_and_focus_app(window_id);
}

pub fn hide_main_window_from_view<V: View>(ctx: &mut ViewContext<V>) {
    ctx.windows().hide_window(ctx.window_id());
}

pub fn show_main_window_from_view<V: View>(ctx: &mut ViewContext<V>) {
    ctx.windows().show_window_and_focus_app(ctx.window_id());
}

pub fn should_hide_main_window_to_tray(
    window_id: WindowId,
    coordinator: &Arc<Mutex<CoordinatorState>>,
) -> bool {
    coordinator
        .lock()
        .ok()
        .is_some_and(|state| state.is_main_shell_window(window_id))
}

pub fn register_main_shell_window(window_id: WindowId, coordinator: &Arc<Mutex<CoordinatorState>>) {
    if let Ok(mut guard) = coordinator.lock() {
        guard.set_main_shell_window(window_id);
    }
}

pub fn quit_desktop<V: View>(ctx: &mut ViewContext<V>) {
    ctx.terminate_app(TerminationMode::Cancellable, None);
}
