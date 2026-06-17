//! RDP viewer top-bar extras: file send, tunnel, terminal, and live audio controls.

use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use warpui::elements::{
    ConstrainedBox, Container, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement, Text,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use warpui_core::keymap::Keystroke;
use wormhole_desktop_rdp::RdpRuntime;

use crate::ui_text;

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
}

impl ExtrasUiState {
    pub fn new() -> Self {
        Self {
            tunnel_local: "127.0.0.1:9999".into(),
            tunnel_remote: "127.0.0.1:3389".into(),
            terminal_cmd: "powershell".into(),
            audio_volume: 100,
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
            .map(|s| (s.virtual_cam_enabled, s.virtual_cam_available, s.virtual_cam_hint.clone()))
            .unwrap_or((false, false, String::new()));
        if vcam.1 {
            let a = on_action.clone();
            row = row.with_child(link_label(
                if vcam.0 { "虚拟摄像头·开" } else { "虚拟摄像头·关" },
                font,
                vcam.0,
                move || a(ExtrasUiAction::ToggleVirtualCam),
            ));
        } else if !vcam.2.is_empty() {
            row = row.with_child(
                ui_text::body(vcam.2, font)
                    .with_color(ColorU::new(140, 140, 140, 255))
                    .finish(),
            );
        }
        let a = on_action.clone();
        row = row.with_child(link_label("断开", font, false, move || {
            a(ExtrasUiAction::Disconnect);
        }));
    }

    row.finish()
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
                    link_label("重新连接", font, false, move || a(ExtrasUiAction::Reconnect))
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

pub fn spawn_send_file(
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    peer: String,
    path: String,
) {
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
            runtime
                .send_file_to_peer(&peer, &path, &transfer_id)
                .await
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
            runtime
                .run_terminal_command(&peer, &command, &[])
                .await
        });
    });
}

pub fn spawn_audio_volume(
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    volume: u8,
) {
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

pub fn spawn_audio_muted(
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    muted: bool,
) {
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
