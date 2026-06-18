//! Workspace-scoped RDP viewer with policy chrome (watermark, no file/tunnel/terminal UI).

use crate::rdp_view::RdpViewerView;
use warpui::ViewContext;
use wormhole_desktop_rdp::RdpRuntime;

pub fn new_workspace_rdp_view(
    ctx: &mut ViewContext<RdpViewerView>,
    runtime: std::sync::Arc<tokio::sync::Mutex<RdpRuntime>>,
    peer: String,
    password: String,
    fps: i32,
    watermark_text: Option<String>,
) -> RdpViewerView {
    RdpViewerView::new_workspace(ctx, runtime, peer, password, fps, watermark_text)
}
