use std::sync::{Arc, Mutex};

use warpui::platform::TerminationMode;
use warpui::{AppContext, View, ViewContext, WindowId};

use crate::coordinator::CoordinatorState;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn prefer_minimize_over_hide() -> bool {
    use warpui::platform::linux::user_windowing_system;
    use warpui::windowing::WindowingSystem;

    user_windowing_system() == WindowingSystem::Wayland
}

pub fn hide_main_window(window_id: WindowId, ctx: &mut AppContext) {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if prefer_minimize_over_hide() {
        if let Some(window) = ctx.windows().platform_window(window_id) {
            window.minimize();
            return;
        }
    }
    ctx.windows().hide_window(window_id);
}

pub fn show_main_window(window_id: WindowId, ctx: &mut AppContext) {
    ctx.windows().show_window_and_focus_app(window_id);
}

pub fn hide_main_window_from_view<V: View>(ctx: &mut ViewContext<V>) {
    let window_id = ctx.window_id();
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if prefer_minimize_over_hide() {
        if let Some(window) = ctx.windows().platform_window(window_id) {
            window.minimize();
            return;
        }
    }
    ctx.windows().hide_window(window_id);
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

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    use warpui::windowing::WindowingSystem;

    #[test]
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn wayland_uses_minimize_hide_strategy() {
        assert!(hide_uses_minimize_for_windowing_system(Some(
            WindowingSystem::Wayland
        )));
        assert!(!hide_uses_minimize_for_windowing_system(Some(
            WindowingSystem::X11
        )));
        assert!(!hide_uses_minimize_for_windowing_system(None));
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn hide_uses_minimize_for_windowing_system(system: Option<WindowingSystem>) -> bool {
        matches!(system, Some(WindowingSystem::Wayland))
    }
}
