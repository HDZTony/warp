//! RDP viewer chrome extras: edge-docked fold panel with toolbar, auth, and tools.

use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::{
    AnchorPair, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Flex, MainAxisAlignment, MainAxisSize, OffsetPositioning,
    OffsetType, ParentElement, ParentOffsetBounds, PositioningAxis, Radius, Rect, XAxisAnchor,
    YAxisAnchor,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use warpui_core::keymap::Keystroke;
use wormhole_desktop_rdp::RdpRuntime;

use crate::ui::panel_primitives::HUD_RADIUS;
use crate::ui::theme;
use crate::ui_text;

/// SavePosition id for the RDP viewer root (window-local chrome drag / snap).
pub const RDP_VIEWER_ROOT_POS: &str = "wormhole-rdp-viewer-root";

const CHROME_ALONG_MIN: f32 = 0.08;
const CHROME_ALONG_MAX: f32 = 0.92;
/// Pointer travel below this (px) counts as a click, not a dock drag.
pub const CHROME_CLICK_SLOP_PX: f32 = 8.0;
const CHROME_EDGE_INSET: f32 = 8.0;
const CHROME_PANEL_INSET: f32 = 12.0;
const CHROME_PANEL_MAX_W: f32 = 520.0;
const CHROME_HANDLE_PAD_X: f32 = 10.0;
const CHROME_HANDLE_PAD_Y: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrasPanel {
    File,
    Tunnel,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewerScale {
    #[default]
    Fit,
    OneToOne,
    Fill,
}

impl ViewerScale {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fit => "适应",
            Self::OneToOne => "1:1",
            Self::Fill => "拉伸",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Fit => Self::OneToOne,
            Self::OneToOne => Self::Fill,
            Self::Fill => Self::Fit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveField {
    #[default]
    FilePath,
    TunnelLocal,
    TunnelRemote,
    TerminalCmd,
    AuthPassword,
    AuthTotp,
}

/// Which window edge the chrome handle (and open panel) docks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChromeEdge {
    Top,
    #[default]
    Right,
    Bottom,
    Left,
}

impl ChromeEdge {
    /// Clamp along-edge ratio so the handle stays away from corners.
    pub fn clamp_along(along: f32) -> f32 {
        along.clamp(CHROME_ALONG_MIN, CHROME_ALONG_MAX)
    }

    /// Snap a point inside a `w`×`h` rect to the nearest edge and along-edge ratio.
    pub fn snap(x: f32, y: f32, w: f32, h: f32) -> (Self, f32) {
        let w = w.max(1.0);
        let h = h.max(1.0);
        let dist_left = x.max(0.0);
        let dist_right = (w - x).max(0.0);
        let dist_top = y.max(0.0);
        let dist_bottom = (h - y).max(0.0);
        if dist_left <= dist_right && dist_left <= dist_top && dist_left <= dist_bottom {
            (Self::Left, Self::clamp_along(y / h))
        } else if dist_right <= dist_top && dist_right <= dist_bottom {
            (Self::Right, Self::clamp_along(y / h))
        } else if dist_top <= dist_bottom {
            (Self::Top, Self::clamp_along(x / w))
        } else {
            (Self::Bottom, Self::clamp_along(x / w))
        }
    }
}

#[derive(Debug, Default)]
pub struct ExtrasUiState {
    pub panel: Option<ExtrasPanel>,
    pub active_field: ActiveField,
    pub file_path: String,
    pub tunnel_local: String,
    pub tunnel_remote: String,
    pub terminal_cmd: String,
    pub audio_muted: bool,
    pub audio_volume: u8,
    pub audio_has_stream: bool,
    pub viewer_scale: ViewerScale,
    pub auth_password: String,
    pub auth_totp: String,
    pub auth_prompt: bool,
    pub virtual_cam_enabled: bool,
    pub virtual_cam_available: bool,
    pub virtual_cam_hint: String,
    pub peer_monitor_count: u32,
    pub peer_monitor_index: u32,
    pub mic_uplink_enabled: bool,
    /// Folded chrome panel visibility (overlay; does not reserve layout height).
    pub chrome_open: bool,
    pub chrome_edge: ChromeEdge,
    /// 0..1 position along the docked edge (clamped away from corners).
    pub chrome_along: f32,
    /// Window-local origin of the current handle press, if any.
    pub chrome_drag_origin: Option<(f32, f32)>,
    /// While dragging, free-float handle center as fractions of the viewer root.
    pub chrome_float_nx: Option<f32>,
    pub chrome_float_ny: Option<f32>,
}

impl ExtrasUiState {
    pub fn new() -> Self {
        Self {
            tunnel_local: "127.0.0.1:9999".into(),
            tunnel_remote: "127.0.0.1:3389".into(),
            terminal_cmd: "powershell".into(),
            audio_volume: 100,
            chrome_along: 0.5,
            chrome_edge: ChromeEdge::Right,
            ..Default::default()
        }
    }

    pub fn active_input_mut(&mut self) -> &mut String {
        match self.active_field {
            ActiveField::FilePath => &mut self.file_path,
            ActiveField::TunnelLocal => &mut self.tunnel_local,
            ActiveField::TunnelRemote => &mut self.tunnel_remote,
            ActiveField::TerminalCmd => &mut self.terminal_cmd,
            ActiveField::AuthPassword => &mut self.auth_password,
            ActiveField::AuthTotp => &mut self.auth_totp,
        }
    }

    pub fn clear_chrome_drag(&mut self) {
        self.chrome_drag_origin = None;
        self.chrome_float_nx = None;
        self.chrome_float_ny = None;
    }
}

#[derive(Debug, Clone)]
pub enum ExtrasUiAction {
    TogglePanel(ExtrasPanel),
    FocusField(ActiveField),
    PickFile,
    Submit,
    ToggleMute,
    VolumeDelta(i8),
    CycleScale,
    Disconnect,
    CtrlAltDel,
    ToggleFullscreen,
    Reconnect,
    ToggleVirtualCam,
    CyclePeerMonitor,
    ToggleMicUplink,
    ToggleChrome,
    BeginChromeDrag {
        x: f32,
        y: f32,
    },
    ChromeDragTo {
        x: f32,
        y: f32,
        root_w: f32,
        root_h: f32,
    },
    EndChromeDrag {
        x: f32,
        y: f32,
        root_w: f32,
        root_h: f32,
    },
}

pub fn apply_keystroke(state: &Arc<Mutex<ExtrasUiState>>, keystroke: &Keystroke) -> bool {
    let submit = keystroke.key == "enter" || keystroke.key == "return";
    if submit {
        return true;
    }
    let mut guard = state.lock().expect("extras ui");
    let input = guard.active_input_mut();
    match keystroke.key.as_str() {
        "backspace" => {
            input.pop();
        }
        "escape" => {
            guard.panel = None;
            if !guard.auth_prompt {
                guard.chrome_open = false;
            }
            guard.clear_chrome_drag();
        }
        key if key.len() == 1 => {
            if let Some(ch) = key.chars().next() {
                input.push(ch);
            }
        }
        _ => {}
    }
    false
}

/// Apply chrome open / drag / snap actions. Returns true when state changed.
pub fn apply_chrome_action(state: &mut ExtrasUiState, action: &ExtrasUiAction) -> bool {
    match action {
        ExtrasUiAction::ToggleChrome => {
            if state.auth_prompt && state.chrome_open {
                return false;
            }
            state.chrome_open = !state.chrome_open;
            state.clear_chrome_drag();
            true
        }
        ExtrasUiAction::BeginChromeDrag { x, y } => {
            state.chrome_drag_origin = Some((*x, *y));
            state.chrome_float_nx = None;
            state.chrome_float_ny = None;
            true
        }
        ExtrasUiAction::ChromeDragTo {
            x,
            y,
            root_w,
            root_h,
        } => {
            if state.chrome_drag_origin.is_none() {
                return false;
            }
            let w = root_w.max(1.0);
            let h = root_h.max(1.0);
            state.chrome_float_nx = Some((*x / w).clamp(0.0, 1.0));
            state.chrome_float_ny = Some((*y / h).clamp(0.0, 1.0));
            true
        }
        ExtrasUiAction::EndChromeDrag {
            x,
            y,
            root_w,
            root_h,
        } => {
            let Some((ox, oy)) = state.chrome_drag_origin else {
                return false;
            };
            let dx = *x - ox;
            let dy = *y - oy;
            let traveled = (dx * dx + dy * dy).sqrt();
            state.clear_chrome_drag();
            if traveled < CHROME_CLICK_SLOP_PX {
                if !(state.auth_prompt && state.chrome_open) {
                    state.chrome_open = !state.chrome_open;
                }
            } else {
                let (edge, along) = ChromeEdge::snap(*x, *y, *root_w, *root_h);
                state.chrome_edge = edge;
                state.chrome_along = along;
            }
            true
        }
        _ => false,
    }
}

fn chrome_bg(open: bool) -> ColorU {
    if open {
        ColorU::new(20, 22, 28, 230)
    } else {
        ColorU::new(20, 22, 28, 160)
    }
}

fn chrome_border(open: bool) -> ColorU {
    if open {
        theme::accent()
    } else {
        ColorU::new(theme::border().r, theme::border().g, theme::border().b, 180)
    }
}

/// Docked or free-floating positioning for the chrome handle / panel.
pub fn chrome_edge_positioning(
    edge: ChromeEdge,
    along: f32,
    float_nx: Option<f32>,
    float_ny: Option<f32>,
    for_panel: bool,
) -> OffsetPositioning {
    let inset = if for_panel {
        CHROME_PANEL_INSET
    } else {
        CHROME_EDGE_INSET
    };
    if let (Some(nx), Some(ny)) = (float_nx, float_ny) {
        return OffsetPositioning::from_axes(
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(nx.clamp(0.0, 1.0)),
                AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Middle),
            ),
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(ny.clamp(0.0, 1.0)),
                AnchorPair::new(YAxisAnchor::Top, YAxisAnchor::Middle),
            ),
        );
    }
    let along = ChromeEdge::clamp_along(along);
    match edge {
        ChromeEdge::Top => OffsetPositioning::from_axes(
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(along),
                AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Middle),
            ),
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Pixel(inset),
                AnchorPair::new(YAxisAnchor::Top, YAxisAnchor::Top),
            ),
        ),
        ChromeEdge::Bottom => OffsetPositioning::from_axes(
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(along),
                AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Middle),
            ),
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Pixel(-inset),
                AnchorPair::new(YAxisAnchor::Bottom, YAxisAnchor::Bottom),
            ),
        ),
        ChromeEdge::Left => OffsetPositioning::from_axes(
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Pixel(inset),
                AnchorPair::new(XAxisAnchor::Left, XAxisAnchor::Left),
            ),
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(along),
                AnchorPair::new(YAxisAnchor::Top, YAxisAnchor::Middle),
            ),
        ),
        ChromeEdge::Right => OffsetPositioning::from_axes(
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Pixel(-inset),
                AnchorPair::new(XAxisAnchor::Right, XAxisAnchor::Right),
            ),
            PositioningAxis::relative_to_parent(
                ParentOffsetBounds::WindowByPosition,
                OffsetType::Percentage(along),
                AnchorPair::new(YAxisAnchor::Top, YAxisAnchor::Middle),
            ),
        ),
    }
}

fn swallow_pointer(child: Box<dyn Element>) -> Box<dyn Element> {
    EventHandler::new(child)
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .on_left_mouse_up(|_, _, _| DispatchEventResult::StopPropagation)
        .on_mouse_dragged(|_, _, _| DispatchEventResult::StopPropagation)
        .on_scroll_wheel(|_, _, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

/// Semi-transparent edge handle: click toggles chrome; drag docks to nearest edge.
pub fn render_chrome_handle(
    font: FamilyId,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> Box<dyn Element> {
    let open = state.lock().map(|g| g.chrome_open).unwrap_or(false);
    let label = if open { "收起" } else { "菜单" };
    let pill = Container::new(
        Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_child(
                ui_text::body(label.to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
    )
    .with_padding_left(CHROME_HANDLE_PAD_X)
    .with_padding_right(CHROME_HANDLE_PAD_X)
    .with_padding_top(CHROME_HANDLE_PAD_Y)
    .with_padding_bottom(CHROME_HANDLE_PAD_Y)
    .with_background(chrome_bg(open))
    .with_border(Border::all(1.0).with_border_fill(chrome_border(open)))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
    .finish();

    let a = on_action;
    EventHandler::new(pill)
        .on_left_mouse_down(move |ctx, _, position| {
            let (ox, oy, _) = root_local_point(ctx, position);
            a(ExtrasUiAction::BeginChromeDrag { x: ox, y: oy });
            DispatchEventResult::StopPropagation
        })
        .finish()
}

/// Resolve mouse position to viewer-root local coords + root size.
pub fn root_local_point(ctx: &warpui::EventContext<'_>, position: Vector2F) -> (f32, f32, (f32, f32)) {
    if let Some(root) = ctx.element_position_by_id(RDP_VIEWER_ROOT_POS) {
        (
            position.x() - root.origin().x(),
            position.y() - root.origin().y(),
            (root.width().max(1.0), root.height().max(1.0)),
        )
    } else {
        (position.x(), position.y(), (1280.0, 720.0))
    }
}

/// Chrome panel content: status + toolbar + optional auth / extras panels.
pub fn render_chrome_panel(
    font: FamilyId,
    mono: FamilyId,
    header_text: String,
    watermark: Option<String>,
    show_tools: bool,
    show_audio: bool,
    watch_only: bool,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> Box<dyn Element> {
    let toolbar = render_toolbar(
        font,
        show_tools,
        show_audio,
        true,
        watch_only,
        state,
        on_action.clone(),
    );
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(
            ui_text::body(header_text, font)
                .with_color(ColorU::white())
                .finish(),
        )
        .with_child(toolbar);
    if let Some(label) = watermark.filter(|s| !s.is_empty()) {
        column = column.with_child(
            ui_text::body(label, font)
                .with_color(ColorU::new(255, 255, 255, 90))
                .finish(),
        );
    }
    if let Some(auth) = render_auth_panel(font, mono, state, on_action.clone()) {
        column = column.with_child(auth);
    }
    if show_tools {
        if let Some(panel) = render_panel(font, mono, state, on_action) {
            column = column.with_child(panel);
        }
    }
    let panel = Container::new(
        ConstrainedBox::new(column.finish())
            .with_max_width(CHROME_PANEL_MAX_W)
            .finish(),
    )
    .with_uniform_padding(10.0)
    .with_background(ColorU::new(20, 22, 28, 220))
    .with_border(Border::all(1.0).with_border_fill(chrome_border(true)))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS + 4.0)))
    .finish();
    swallow_pointer(panel)
}

/// Full-window scrim that closes chrome when clicked (above video, below panel).
pub fn render_chrome_scrim(on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>) -> Box<dyn Element> {
    EventHandler::new(
        Rect::new()
            .with_background_color(ColorU::new(0, 0, 0, 1))
            .finish(),
    )
    .on_left_mouse_down(move |_, _, _| {
        on_action(ExtrasUiAction::ToggleChrome);
        DispatchEventResult::StopPropagation
    })
    .finish()
}

/// Convenience: handle element + its [`OffsetPositioning`] for the current chrome state.
pub fn chrome_handle_with_position(
    font: FamilyId,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> (Box<dyn Element>, OffsetPositioning) {
    let (edge, along, float_nx, float_ny) = state
        .lock()
        .map(|g| {
            (
                g.chrome_edge,
                g.chrome_along,
                g.chrome_float_nx,
                g.chrome_float_ny,
            )
        })
        .unwrap_or((ChromeEdge::Right, 0.5, None, None));
    let handle = render_chrome_handle(font, state, on_action);
    let pos = chrome_edge_positioning(edge, along, float_nx, float_ny, false);
    (handle, pos)
}

/// Convenience: open chrome panel + positioning (same edge as handle; not free-floating).
pub fn chrome_panel_with_position(
    font: FamilyId,
    mono: FamilyId,
    header_text: String,
    watermark: Option<String>,
    show_tools: bool,
    show_audio: bool,
    watch_only: bool,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> (Box<dyn Element>, OffsetPositioning) {
    let (edge, along) = state
        .lock()
        .map(|g| (g.chrome_edge, g.chrome_along))
        .unwrap_or((ChromeEdge::Right, 0.5));
    let panel = render_chrome_panel(
        font,
        mono,
        header_text,
        watermark,
        show_tools,
        show_audio,
        watch_only,
        state,
        on_action,
    );
    let pos = chrome_edge_positioning(edge, along, None, None, true);
    (panel, pos)
}

pub fn link_label(
    label: &str,
    font: FamilyId,
    active: bool,
    on_click: impl Fn() + Send + Sync + 'static,
) -> Box<dyn Element> {
    let color = if active {
        ColorU::new(255, 220, 120, 255)
    } else {
        ColorU::new(140, 190, 255, 255)
    };
    EventHandler::new(
        Container::new(
            ui_text::body(label.to_string(), font)
                .with_color(color)
                .finish(),
        )
        .with_uniform_padding(4.)
        .finish(),
    )
    .on_left_mouse_up(move |_, _, _| {
        on_click();
        DispatchEventResult::StopPropagation
    })
    .finish()
}

pub fn render_toolbar(
    font: FamilyId,
    show_tools: bool,
    show_audio: bool,
    show_viewer: bool,
    watch_only: bool,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> Box<dyn Element> {
    let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);

    if show_tools {
        let panel = state.lock().map(|s| s.panel).ok().flatten();
        let file_active = panel == Some(ExtrasPanel::File);
        let tunnel_active = panel == Some(ExtrasPanel::Tunnel);
        let term_active = panel == Some(ExtrasPanel::Terminal);
        let a = on_action.clone();
        row = row.with_child(link_label("发文件", font, file_active, move || {
            a(ExtrasUiAction::TogglePanel(ExtrasPanel::File));
        }));
        let a = on_action.clone();
        row = row.with_child(link_label("隧道", font, tunnel_active, move || {
            a(ExtrasUiAction::TogglePanel(ExtrasPanel::Tunnel));
        }));
        let a = on_action.clone();
        row = row.with_child(link_label("终端", font, term_active, move || {
            a(ExtrasUiAction::TogglePanel(ExtrasPanel::Terminal));
        }));
    }

    if show_audio {
        let (muted, volume, has_stream) = state
            .lock()
            .map(|s| (s.audio_muted, s.audio_volume, s.audio_has_stream))
            .unwrap_or((false, 100, false));
        let mute_label = if muted {
            "静音".to_string()
        } else if has_stream {
            format!("音量 {volume}%")
        } else {
            "等待音频…".to_string()
        };
        let a = on_action.clone();
        row = row.with_child(link_label(
            if muted { "取消静音" } else { "静音" },
            font,
            muted,
            move || a(ExtrasUiAction::ToggleMute),
        ));
        row = row.with_child(
            ui_text::body(mute_label, font)
                .with_color(ColorU::new(180, 180, 180, 255))
                .finish(),
        );
        let a = on_action.clone();
        row = row.with_child(link_label("Vol-", font, false, move || {
            a(ExtrasUiAction::VolumeDelta(-10));
        }));
        let a = on_action.clone();
        row = row.with_child(link_label("Vol+", font, false, move || {
            a(ExtrasUiAction::VolumeDelta(10));
        }));
    }

    if show_viewer {
        let scale_label = state
            .lock()
            .map(|s| s.viewer_scale.label().to_string())
            .unwrap_or_else(|_| ViewerScale::Fit.label().to_string());
        let a = on_action.clone();
        row = row.with_child(link_label(
            &format!("缩放·{scale_label}"),
            font,
            false,
            move || a(ExtrasUiAction::CycleScale),
        ));
        let a = on_action.clone();
        row = row.with_child(link_label("全屏", font, false, move || {
            a(ExtrasUiAction::ToggleFullscreen);
        }));
        if !watch_only {
            #[cfg(windows)]
            {
                let a = on_action.clone();
                row = row.with_child(link_label("Ctrl+Alt+Del", font, false, move || {
                    a(ExtrasUiAction::CtrlAltDel);
                }));
            }
        }
        let vcam = state
            .lock()
            .map(|s| {
                (
                    s.virtual_cam_enabled,
                    s.virtual_cam_available,
                    s.virtual_cam_hint.clone(),
                )
            })
            .unwrap_or((false, false, String::new()));
        if vcam.1 {
            let a = on_action.clone();
            row = row.with_child(link_label(
                if vcam.0 {
                    "虚拟摄像头·开"
                } else {
                    "虚拟摄像头·关"
                },
                font,
                vcam.0,
                move || a(ExtrasUiAction::ToggleVirtualCam),
            ));
        } else if !vcam.2.is_empty() {
            row = row.with_child(render_virtual_cam_guidance(font, &vcam.2));
        }
        let (mon_count, mon_idx, mic_on) = state
            .lock()
            .map(|s| {
                (
                    s.peer_monitor_count,
                    s.peer_monitor_index,
                    s.mic_uplink_enabled,
                )
            })
            .unwrap_or((0, 0, false));
        if mon_count > 1 {
            let a = on_action.clone();
            row = row.with_child(link_label(
                &format!("屏幕 {}/{}", mon_idx.saturating_add(1), mon_count),
                font,
                false,
                move || a(ExtrasUiAction::CyclePeerMonitor),
            ));
        }
        let a = on_action.clone();
        row = row.with_child(link_label(
            if mic_on {
                "上行麦·开"
            } else {
                "上行麦·关"
            },
            font,
            mic_on,
            move || a(ExtrasUiAction::ToggleMicUplink),
        ));
        let a = on_action.clone();
        row = row.with_child(link_label("断开", font, false, move || {
            a(ExtrasUiAction::Disconnect);
        }));
    }

    row.finish()
}

fn render_virtual_cam_guidance(font: FamilyId, hint: &str) -> Box<dyn Element> {
    #[cfg(target_os = "macos")]
    if hint.contains("OBS") || hint.contains("BlackHole") {
        return Container::new(
            Flex::column()
                .with_child(
                    ui_text::body("macOS 虚拟摄像头", font)
                        .with_color(ColorU::new(210, 210, 210, 255))
                        .finish(),
                )
                .with_child(guidance_line(
                    font,
                    "1. 安装 OBS Studio，菜单「工具 → 虚拟摄像头」启动输出。",
                ))
                .with_child(guidance_line(
                    font,
                    "2. 在 Zoom / Teams / Meet 中选择「OBS Virtual Camera」。",
                ))
                .with_child(guidance_line(
                    font,
                    "3. 若需把 Viewer 音频注入会议麦，另装 BlackHole 并在系统声音里选多输出。",
                ))
                .finish(),
        )
        .with_uniform_padding(4.)
        .finish();
    }
    Container::new(
        ui_text::body(hint.to_string(), font)
            .with_color(ColorU::new(140, 140, 140, 255))
            .finish(),
    )
    .with_uniform_padding(4.)
    .finish()
}

fn guidance_line(font: FamilyId, text: &str) -> Box<dyn Element> {
    Container::new(
        ui_text::body(text.to_string(), font)
            .with_color(ColorU::new(140, 140, 140, 255))
            .finish(),
    )
    .with_padding_top(2.)
    .finish()
}

pub fn render_auth_panel(
    font: FamilyId,
    mono: FamilyId,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> Option<Box<dyn Element>> {
    let guard = state.lock().ok()?;
    if !guard.auth_prompt {
        return None;
    }
    let password = guard.auth_password.clone();
    let totp = guard.auth_totp.clone();
    let a = on_action.clone();
    Some(
        Container::new(
            Flex::column()
                .with_child(
                    ui_text::body("需要连接密码或 TOTP", font)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .with_child(
                    ui_text::mono(format!("密码：{password}"), mono)
                        .with_color(ColorU::new(220, 220, 220, 255))
                        .finish(),
                )
                .with_child(link_label("编辑密码", font, true, move || {
                    a(ExtrasUiAction::FocusField(ActiveField::AuthPassword));
                }))
                .with_child(
                    ui_text::mono(format!("TOTP：{totp}"), mono)
                        .with_color(ColorU::new(220, 220, 220, 255))
                        .finish(),
                )
                .with_child({
                    let a = on_action.clone();
                    link_label("编辑 TOTP", font, false, move || {
                        a(ExtrasUiAction::FocusField(ActiveField::AuthTotp));
                    })
                })
                .with_child({
                    let a = on_action.clone();
                    link_label("重新连接", font, false, move || {
                        a(ExtrasUiAction::Reconnect)
                    })
                })
                .with_child(
                    ui_text::body("Enter 重新连接", font)
                        .with_color(ColorU::new(140, 140, 140, 255))
                        .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(6.)
        .finish(),
    )
}

pub fn render_panel(
    font: FamilyId,
    mono: FamilyId,
    state: &Arc<Mutex<ExtrasUiState>>,
    on_action: Arc<dyn Fn(ExtrasUiAction) + Send + Sync>,
) -> Option<Box<dyn Element>> {
    let guard = state.lock().ok()?;
    let panel = guard.panel?;
    let hint = "Enter 提交 · Esc 关闭面板";
    let body = match panel {
        ExtrasPanel::File => {
            let path = guard.file_path.clone();
            Flex::column()
                .with_child(
                    ui_text::mono(format!("本地文件路径：{path}"), mono)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .with_child(link_label("选择字段", font, true, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::FocusField(ActiveField::FilePath))
                }))
                .with_child(link_label("发送", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::Submit)
                }))
                .finish()
        }
        ExtrasPanel::Tunnel => {
            let local = guard.tunnel_local.clone();
            let remote = guard.tunnel_remote.clone();
            Flex::column()
                .with_child(
                    ui_text::mono(format!("本地绑定：{local}"), mono)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .with_child(
                    ui_text::mono(format!("远端目标：{remote}"), mono)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .with_child(link_label("编辑本地", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::FocusField(ActiveField::TunnelLocal))
                }))
                .with_child(link_label("编辑远端", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::FocusField(ActiveField::TunnelRemote))
                }))
                .with_child(link_label("打开隧道", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::Submit)
                }))
                .finish()
        }
        ExtrasPanel::Terminal => {
            let cmd = guard.terminal_cmd.clone();
            Flex::column()
                .with_child(
                    ui_text::mono(format!("命令：{cmd}"), mono)
                        .with_color(ColorU::white())
                        .finish(),
                )
                .with_child(link_label("编辑命令", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::FocusField(ActiveField::TerminalCmd))
                }))
                .with_child(link_label("运行", font, false, {
                    let a = on_action.clone();
                    move || a(ExtrasUiAction::Submit)
                }))
                .finish()
        }
    };
    drop(guard);

    Some(
        Container::new(
            ConstrainedBox::new(
                Flex::column()
                    .with_child(body)
                    .with_child(
                        ui_text::body(hint.to_string(), font)
                            .with_color(ColorU::new(140, 140, 140, 255))
                            .finish(),
                    )
                    .finish(),
            )
            .with_height(88.)
            .finish(),
        )
        .with_uniform_padding(6.)
        .with_background_color(ColorU::new(24, 28, 36, 240))
        .finish(),
    )
}

pub fn spawn_send_file(runtime: Arc<tokio::sync::Mutex<RdpRuntime>>, peer: String, path: String) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                tracing::warn!("extras file runtime: {err}");
                return;
            }
        };
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let _ = rt.block_on(async move {
            let runtime = runtime.lock().await;
            runtime.send_file_to_peer(&peer, &path, &transfer_id).await
        });
    });
}

pub fn spawn_open_tunnel(
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    peer: String,
    local_bind: String,
    remote: String,
) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                tracing::warn!("extras tunnel runtime: {err}");
                return;
            }
        };
        let (remote_host, remote_port) = parse_tunnel_remote(&remote);
        let _ = rt.block_on(async move {
            let runtime = runtime.lock().await;
            runtime
                .open_tunnel(&peer, &local_bind, &remote_host, remote_port)
                .await
        });
    });
}

pub fn spawn_run_terminal(
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    peer: String,
    command: String,
) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                tracing::warn!("extras terminal runtime: {err}");
                return;
            }
        };
        let _ = rt.block_on(async move {
            let runtime = runtime.lock().await;
            runtime.run_terminal_command(&peer, &command, &[]).await
        });
    });
}

pub fn spawn_audio_volume(runtime: Arc<tokio::sync::Mutex<RdpRuntime>>, volume: u8) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return,
        };
        let _ = rt.block_on(async move {
            let runtime = runtime.lock().await;
            runtime.set_viewer_audio_volume(volume).await
        });
    });
}

pub fn spawn_audio_muted(runtime: Arc<tokio::sync::Mutex<RdpRuntime>>, muted: bool) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return,
        };
        let _ = rt.block_on(async move {
            let runtime = runtime.lock().await;
            runtime.set_viewer_audio_muted(muted).await
        });
    });
}

fn parse_tunnel_remote(remote: &str) -> (String, u16) {
    let remote = remote.trim();
    if let Some((host, port)) = remote.rsplit_once(':') {
        let port = port.parse().unwrap_or(3389);
        return (host.to_string(), port);
    }
    (remote.to_string(), 3389)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_edge_snap_picks_nearest_side() {
        let (edge, along) = ChromeEdge::snap(10.0, 200.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Left);
        assert!((along - 200.0 / 600.0).abs() < 0.001);

        let (edge, along) = ChromeEdge::snap(790.0, 300.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Right);
        assert!((along - 0.5).abs() < 0.001);

        let (edge, along) = ChromeEdge::snap(400.0, 5.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Top);
        assert!((along - 0.5).abs() < 0.001);

        let (edge, along) = ChromeEdge::snap(400.0, 595.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Bottom);
        assert!((along - 0.5).abs() < 0.001);
    }

    #[test]
    fn chrome_edge_snap_clamps_along_away_from_corners() {
        let (edge, along) = ChromeEdge::snap(5.0, 1.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Top);
        assert!((along - CHROME_ALONG_MIN).abs() < f32::EPSILON);

        let (edge, along) = ChromeEdge::snap(795.0, 599.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Bottom);
        assert!((along - CHROME_ALONG_MAX).abs() < f32::EPSILON);

        let (edge, along) = ChromeEdge::snap(1.0, 300.0, 800.0, 600.0);
        assert_eq!(edge, ChromeEdge::Left);
        assert!((along - 0.5).abs() < 0.001);
    }

    #[test]
    fn apply_chrome_action_click_toggles_open() {
        let mut state = ExtrasUiState::new();
        assert!(!state.chrome_open);
        assert!(apply_chrome_action(
            &mut state,
            &ExtrasUiAction::BeginChromeDrag { x: 10.0, y: 10.0 }
        ));
        assert!(apply_chrome_action(
            &mut state,
            &ExtrasUiAction::EndChromeDrag {
                x: 12.0,
                y: 11.0,
                root_w: 800.0,
                root_h: 600.0,
            }
        ));
        assert!(state.chrome_open);
        assert!(state.chrome_drag_origin.is_none());
    }

    #[test]
    fn apply_chrome_action_drag_snaps_edge() {
        let mut state = ExtrasUiState::new();
        assert!(apply_chrome_action(
            &mut state,
            &ExtrasUiAction::BeginChromeDrag { x: 400.0, y: 300.0 }
        ));
        assert!(apply_chrome_action(
            &mut state,
            &ExtrasUiAction::ChromeDragTo {
                x: 20.0,
                y: 300.0,
                root_w: 800.0,
                root_h: 600.0,
            }
        ));
        assert!(state.chrome_float_nx.is_some());
        assert!(apply_chrome_action(
            &mut state,
            &ExtrasUiAction::EndChromeDrag {
                x: 20.0,
                y: 300.0,
                root_w: 800.0,
                root_h: 600.0,
            }
        ));
        assert_eq!(state.chrome_edge, ChromeEdge::Left);
        assert!(state.chrome_float_nx.is_none());
        assert!(!state.chrome_open);
    }
}
