use pathfinder_geometry::vector::Vector2F;
use warpui::platform::WindowBounds;
use warpui::AddWindowOptions;

/// Main window: frameless with one dark chrome row (tabs + caption buttons in `app_shell`).
pub fn desktop_window_options(title: impl Into<String>, size: Vector2F) -> AddWindowOptions {
    AddWindowOptions {
        title: Some(title.into()),
        window_bounds: WindowBounds::ExactSize(size),
        hide_title_bar: true,
        ..Default::default()
    }
}

/// Secondary windows (RDP, HUD, …): native title bar; no custom tab chrome.
pub fn desktop_popout_window_options(title: impl Into<String>, size: Vector2F) -> AddWindowOptions {
    AddWindowOptions {
        title: Some(title.into()),
        window_bounds: WindowBounds::ExactSize(size),
        hide_title_bar: false,
        ..Default::default()
    }
}
