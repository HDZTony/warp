use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pathfinder_color::ColorU;
use warpui::elements::{
    Container, DispatchEventResult, EventHandler, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::{Cache as FontCache, FamilyId};
use warpui::{
    AppContext, Element, Entity, SingletonEntity as _, TypedActionView, View, ViewContext,
};
use warpui_core::keymap::Keystroke;
use wormhole_desktop_rdp::settings::{load_settings, save_settings, RdpSettings};
use wormhole_desktop_rdp::wol::send_magic_packet;
use wormhole_desktop_rdp::{
    apply_host_side_effects, format_addressbook_entries, format_entries, generate_totp_secret,
    list as list_audit, load_addressbook, logon_task_installed, trim as trim_audit,
    upsert_addressbook_entry, windows_service_installed, RdpAddressBookEntry, RdpRuntime,
    RemoteDesktopSessionDto, SessionRole, DEFAULT_BROADCAST,
};

use crate::coordinator::{CoordinatorState, UiCommand};
use crate::rdp_extras_ui::link_label;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostControlPage {
    Host,
    Connect,
    Book,
    Tools,
    Sessions,
    Audit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostControlField {
    ConnectHost,
    ConnectPassword,
    ConnectTotp,
    HostPassword,
    AbName,
    AbNodeId,
    WolMac,
}

struct HostControlUi {
    page: HostControlPage,
    active_field: HostControlField,
    settings: RdpSettings,
    node_id: String,
    sessions: Vec<RemoteDesktopSessionDto>,
    sessions_text: String,
    status: String,
    connect_host: String,
    connect_password: String,
    connect_totp: String,
    ab_name: String,
    ab_node_id: String,
    wol_mac: String,
    book_entries: Vec<RdpAddressBookEntry>,
    discovered_text: String,
    audit_text: String,
    supported_codecs: Vec<String>,
    vram_available: bool,
    logon_task_installed: bool,
    windows_service_installed: bool,
}

impl Default for HostControlUi {
    fn default() -> Self {
        Self {
            page: HostControlPage::Host,
            active_field: HostControlField::ConnectHost,
            settings: RdpSettings::default(),
            node_id: "…".into(),
            sessions: Vec::new(),
            sessions_text: "加载会话…".into(),
            status: String::new(),
            connect_host: String::new(),
            connect_password: String::new(),
            connect_totp: String::new(),
            ab_name: String::new(),
            ab_node_id: String::new(),
            wol_mac: String::new(),
            book_entries: Vec::new(),
            discovered_text: String::new(),
            audit_text: "加载审计…".into(),
            supported_codecs: vec!["h264".into()],
            vram_available: false,
            logon_task_installed: false,
            windows_service_installed: false,
        }
    }
}

pub struct RdpHostControlView {
    data_dir: PathBuf,
    runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
    coordinator: Arc<Mutex<CoordinatorState>>,
    ui: Arc<Mutex<HostControlUi>>,
    generation: Arc<Mutex<u64>>,
    font: FamilyId,
    mono: FamilyId,
}

impl RdpHostControlView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        data_dir: PathBuf,
        runtime: Arc<tokio::sync::Mutex<RdpRuntime>>,
        coordinator: Arc<Mutex<CoordinatorState>>,
    ) -> Self {
        let font = FontCache::handle(ctx)
            .update(ctx, |cache, _| cache.load_system_font("Segoe UI").ok())
            .unwrap_or(FamilyId(0));
        let mono = FontCache::handle(ctx)
            .update(ctx, |cache, _| {
                cache
                    .load_system_font("Consolas")
                    .or_else(|_| cache.load_system_font("Cascadia Mono"))
                    .ok()
            })
            .unwrap_or(font);

        let view = Self {
            data_dir,
            runtime,
            coordinator,
            ui: Arc::new(Mutex::new(HostControlUi::default())),
            generation: Arc::new(Mutex::new(1)),
            font,
            mono,
        };
        view.poll_once(ctx);
        view.start_ui_poll(ctx);
        ctx.focus_self();
        view
    }

    fn start_ui_poll(&self, ctx: &mut ViewContext<Self>) {
        let generation = self.generation.clone();
        let last = Arc::new(Mutex::new(0u64));
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::ui_poll_once(ctx, tick_rx, generation, last);
    }

    fn ui_poll_once(
        ctx: &mut ViewContext<Self>,
        tick_rx: async_channel::Receiver<()>,
        generation: Arc<Mutex<u64>>,
        last: Arc<Mutex<u64>>,
    ) {
        let waiter = tick_rx.clone();
        ctx.spawn(async move { waiter.recv().await }, move |_, output, ctx| {
            if output.is_ok() {
                let current = generation.lock().map(|g| *g).unwrap_or(0);
                let prev = last.lock().map(|g| *g).unwrap_or(0);
                if current != prev {
                    if let Ok(mut guard) = last.lock() {
                        *guard = current;
                    }
                    ctx.notify();
                }
                Self::ui_poll_once(ctx, tick_rx, generation, last);
            }
        });
    }

    fn bump(&self) {
        if let Ok(mut g) = self.generation.lock() {
            *g = g.saturating_add(1);
        }
    }

    fn poll_once(&self, ctx: &mut ViewContext<Self>) {
        let (tick_tx, tick_rx) = async_channel::unbounded::<()>();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            if tick_tx.send_blocking(()).is_err() {
                break;
            }
        });
        Self::schedule_poll(ctx, tick_rx);
    }

    fn schedule_poll(ctx: &mut ViewContext<Self>, tick_rx: async_channel::Receiver<()>) {
        let waiter = tick_rx.clone();
        ctx.spawn(
            async move { waiter.recv().await },
            move |view, output, ctx| {
                if output.is_ok() {
                    view.refresh_data();
                    view.bump();
                    ctx.notify();
                    Self::schedule_poll(ctx, tick_rx);
                }
            },
        );
    }

    fn refresh_data(&self) {
        let data_dir = self.data_dir.clone();
        let runtime = self.runtime.clone();
        let snapshot = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok()?;
            rt.block_on(async move {
                let settings = load_settings(&data_dir).await.ok()?;
                let runtime = runtime.lock().await;
                let config = runtime.remote_desktop_config().await.ok()?;
                let sessions = runtime.list_sessions().await.ok()?;
                let audit = list_audit(&data_dir, 200)
                    .await
                    .map(|entries| format_entries(&entries))
                    .unwrap_or_else(|e| format!("读取审计失败: {e}"));
                let book = load_addressbook(&data_dir).await.unwrap_or_default();
                let discovered_text = format_discovered(&sessions);
                Some((
                    settings,
                    config.node_id.unwrap_or_default(),
                    config.supported_codecs,
                    config.vram_available,
                    sessions,
                    audit,
                    book.entries,
                    discovered_text,
                ))
            })
        })
        .join()
        .ok()
        .flatten();

        if let Some((
            settings,
            node_id,
            supported_codecs,
            vram_available,
            sessions,
            audit_text,
            book_entries,
            discovered_text,
        )) = snapshot
        {
            let sessions_text = format_sessions(&sessions);
            if let Ok(mut ui) = self.ui.lock() {
                ui.settings = settings;
                ui.node_id = if node_id.is_empty() {
                    "（未初始化）".into()
                } else {
                    node_id
                };
                ui.supported_codecs = if supported_codecs.is_empty() {
                    vec!["h264".into()]
                } else {
                    supported_codecs
                };
                ui.vram_available = vram_available;
                ui.logon_task_installed = logon_task_installed();
                ui.windows_service_installed = windows_service_installed();
                ui.sessions = sessions;
                ui.sessions_text = sessions_text;
                ui.audit_text = audit_text;
                ui.book_entries = book_entries;
                ui.discovered_text = discovered_text;
            }
        }
    }

    fn trim_audit_log(&self) {
        let data_dir = self.data_dir.clone();
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = match rt {
                Some(rt) => match rt.block_on(async {
                    trim_audit(&data_dir).await?;
                    let entries = list_audit(&data_dir, 200).await?;
                    Ok::<_, String>(format_entries(&entries))
                }) {
                    Ok(text) => format!("已修剪审计日志\n────────\n{text}"),
                    Err(e) => format!("修剪审计失败: {e}"),
                },
                None => "修剪审计日志失败".to_string(),
            };
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg.clone();
                if let Some(body) = msg.split_once("\n────────\n").map(|(_, b)| b) {
                    guard.audit_text = body.to_string();
                }
            }
        });
        self.bump();
    }

    fn refresh_audit_now(&self) {
        let data_dir = self.data_dir.clone();
        let ui = self.ui.clone();
        let gen = self.generation.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let text = match rt {
                Some(rt) => match rt.block_on(async {
                    list_audit(&data_dir, 200)
                        .await
                        .map(|entries| format_entries(&entries))
                }) {
                    Ok(text) => text,
                    Err(e) => format!("读取审计失败: {e}"),
                },
                None => "读取审计失败".into(),
            };
            if let Ok(mut guard) = ui.lock() {
                guard.audit_text = text;
                guard.status = "审计已刷新".into();
            }
            if let Ok(mut g) = gen.lock() {
                *g = g.saturating_add(1);
            }
        });
    }

    fn stop_session(&self, session_id: String) {
        let runtime = self.runtime.clone();
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = rt
                .and_then(|rt| {
                    rt.block_on(async {
                        let runtime = runtime.lock().await;
                        runtime.stop_session(&session_id).await
                    })
                    .ok()
                })
                .map(|_| format!("已停止 {session_id}"))
                .unwrap_or_else(|| "停止会话失败".to_string());
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
        });
        self.bump();
    }

    fn save_settings(&self) {
        let data_dir = self.data_dir.clone();
        let settings = self
            .ui
            .lock()
            .map(|u| u.settings.clone())
            .unwrap_or_default();
        let runtime = self.runtime.clone();
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = rt
                .and_then(|rt| {
                    rt.block_on(async {
                        save_settings(&data_dir, &settings).await?;
                        apply_host_side_effects(&data_dir, &settings)?;
                        let runtime = runtime.lock().await;
                        runtime.ensure_unattended_host(&settings).await
                    })
                    .ok()
                })
                .map(|_| "设置已保存".to_string())
                .unwrap_or_else(|| "保存设置失败".to_string());
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
                guard.logon_task_installed = logon_task_installed();
                guard.windows_service_installed = windows_service_installed();
            }
        });
        self.bump();
    }

    fn start_sharing(&self) {
        let runtime = self.runtime.clone();
        let ui = self.ui.clone();
        let (fps, use_vram, monitor) = self
            .ui
            .lock()
            .map(|u| {
                (
                    u.settings.host_fps,
                    u.settings.host_use_vram,
                    u.settings.host_monitor,
                )
            })
            .unwrap_or((30, false, 0));
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = rt
                .and_then(|rt| {
                    rt.block_on(async {
                        let runtime = runtime.lock().await;
                        runtime.accept_host(fps, use_vram, monitor).await
                    })
                    .ok()
                })
                .map(|id| format!("Host 已启动 · session {id}"))
                .unwrap_or_else(|| "启动 Host 失败".to_string());
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
        });
        self.bump();
    }

    fn connect(&self) {
        let host = self
            .ui
            .lock()
            .map(|u| u.connect_host.clone())
            .unwrap_or_default();
        self.open_peer(host);
    }

    fn connect_from_book(&self, peer: String) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.connect_host = peer.clone();
            ui.page = HostControlPage::Connect;
        }
        self.open_peer(peer);
    }

    fn connect_discovered(&self, peer: String, requires_password: bool) {
        if requires_password {
            let has_password = self
                .ui
                .lock()
                .map(|u| !u.connect_password.trim().is_empty())
                .unwrap_or(false);
            if !has_password {
                if let Ok(mut u) = self.ui.lock() {
                    u.connect_host = peer;
                    u.page = HostControlPage::Connect;
                    u.active_field = HostControlField::ConnectPassword;
                    u.status = "该主机需要连接密码".into();
                }
                self.bump();
                return;
            }
        }
        if let Ok(mut ui) = self.ui.lock() {
            ui.connect_host = peer.clone();
            ui.page = HostControlPage::Connect;
        }
        self.open_peer(peer);
    }

    fn add_discovered_to_book(&self, node_id: String, name: String) {
        let name = name.trim().to_string();
        let node_id = node_id.trim().to_string();
        if name.is_empty() || node_id.is_empty() {
            return;
        }
        let data_dir = self.data_dir.clone();
        let ui = self.ui.clone();
        let gen = self.generation.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = match rt {
                Some(rt) => match rt.block_on(async {
                    upsert_addressbook_entry(&data_dir, name, node_id, Vec::new(), true).await?;
                    load_addressbook(&data_dir).await
                }) {
                    Ok(book) => {
                        if let Ok(mut guard) = ui.lock() {
                            guard.book_entries = book.entries;
                        }
                        "已加入地址簿".to_string()
                    }
                    Err(e) => format!("加入地址簿失败: {e}"),
                },
                None => "加入地址簿失败".to_string(),
            };
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
            if let Ok(mut g) = gen.lock() {
                *g = g.saturating_add(1);
            }
        });
        self.bump();
    }

    fn open_peer(&self, host: String) {
        let host = host.trim().to_string();
        if host.is_empty() {
            if let Ok(mut u) = self.ui.lock() {
                u.status = "请先填写 Host Node ID".into();
            }
            self.bump();
            return;
        }
        let (password, totp) = self
            .ui
            .lock()
            .map(|u| (u.connect_password.clone(), u.connect_totp.clone()))
            .unwrap_or_default();
        let window_key = wormhole_native_ipc::rdp_window_key(&host);
        let title = format!("RDP · {host}");
        let password = if password.trim().is_empty() {
            None
        } else {
            Some(password)
        };
        let totp_code = if totp.trim().is_empty() {
            None
        } else {
            Some(totp)
        };
        if let Ok(mut guard) = self.coordinator.lock() {
            guard.enqueue(UiCommand::OpenRdp {
                peer: host,
                title,
                reconnect: false,
                window_key,
                password,
                totp_code,
                fps: 60,
            });
        }
        if let Ok(mut u) = self.ui.lock() {
            u.status = "正在打开原生 RDP 窗…".into();
        }
        self.bump();
    }

    fn save_address_book_entry(&self) {
        let (name, node_id) = self
            .ui
            .lock()
            .map(|u| (u.ab_name.clone(), u.ab_node_id.clone()))
            .unwrap_or_default();
        let name = name.trim().to_string();
        let node_id = node_id.trim().to_string();
        if name.is_empty() || node_id.is_empty() {
            if let Ok(mut u) = self.ui.lock() {
                u.status = "地址簿：请填写名称与 Node ID".into();
            }
            self.bump();
            return;
        }
        let data_dir = self.data_dir.clone();
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = match rt {
                Some(rt) => match rt.block_on(async {
                    upsert_addressbook_entry(&data_dir, name, node_id, Vec::new(), true).await?;
                    load_addressbook(&data_dir).await
                }) {
                    Ok(book) => {
                        if let Ok(mut guard) = ui.lock() {
                            guard.book_entries = book.entries;
                            guard.ab_name.clear();
                            guard.ab_node_id.clear();
                        }
                        "已保存到地址簿".to_string()
                    }
                    Err(e) => format!("保存地址簿失败: {e}"),
                },
                None => "保存地址簿失败".to_string(),
            };
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
        });
        self.bump();
    }

    fn send_wol(&self) {
        let mac = self
            .ui
            .lock()
            .map(|u| u.wol_mac.clone())
            .unwrap_or_default();
        let mac = mac.trim().to_string();
        if mac.is_empty() {
            if let Ok(mut u) = self.ui.lock() {
                u.status = "请填写 MAC 地址（AA:BB:CC:DD:EE:FF）".into();
            }
            self.bump();
            return;
        }
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let msg = match send_magic_packet(&mac, DEFAULT_BROADCAST) {
                Ok(()) => "Wake-on-LAN 魔术包已发送".to_string(),
                Err(e) => format!("WoL 失败: {e}"),
            };
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
        });
        self.bump();
    }

    fn stop_all(&self) {
        let runtime = self.runtime.clone();
        let ui = self.ui.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().ok();
            let msg = rt
                .and_then(|rt| {
                    rt.block_on(async {
                        let runtime = runtime.lock().await;
                        runtime.stop_all_sessions().await
                    })
                    .ok()
                })
                .map(|_| "已停止全部会话".to_string())
                .unwrap_or_else(|| "停止失败".to_string());
            if let Ok(mut guard) = ui.lock() {
                guard.status = msg;
            }
        });
        self.bump();
    }

    fn toggle_unattended(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.unattended_enabled = !ui.settings.unattended_enabled;
        }
        self.bump();
    }

    fn toggle_auto_start(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.auto_start_on_launch = !ui.settings.auto_start_on_launch;
        }
        self.bump();
    }

    fn toggle_vram(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.host_use_vram = !ui.settings.host_use_vram;
        }
        self.bump();
    }

    fn toggle_privacy_screen(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.privacy_screen = !ui.settings.privacy_screen;
        }
        self.bump();
    }

    fn toggle_totp_required(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.totp_required = !ui.settings.totp_required;
        }
        self.bump();
    }

    fn toggle_logon_task(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.windows_logon_task = !ui.settings.windows_logon_task;
        }
        self.bump();
    }

    fn toggle_windows_service_mode(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.windows_service_mode = !ui.settings.windows_service_mode;
        }
        self.bump();
    }

    fn cycle_quality(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.quality_preset = next_quality_preset(&ui.settings.quality_preset);
        }
        self.bump();
    }

    fn cycle_codec(&self) {
        if let Ok(mut ui) = self.ui.lock() {
            let codecs = ui.supported_codecs.clone();
            ui.settings.host_codec = next_host_codec(&ui.settings.host_codec, &codecs);
        }
        self.bump();
    }

    fn bump_fps(&self, delta: i32) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.host_fps = (ui.settings.host_fps + delta).clamp(1, 240);
        }
        self.bump();
    }

    fn bump_monitor(&self, delta: i32) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.host_monitor = (ui.settings.host_monitor + delta).clamp(0, 7);
        }
        self.bump();
    }

    fn generate_totp(&self) {
        let secret = generate_totp_secret();
        if let Ok(mut ui) = self.ui.lock() {
            ui.settings.totp_secret = Some(secret.clone());
            ui.settings.totp_required = true;
            ui.status = format!("TOTP 密钥已生成：{secret}\n请保存设置并备份密钥。");
        }
        self.bump();
    }

    fn focus_field(&self, field: HostControlField) {
        if let Ok(mut ui) = self.ui.lock() {
            ui.active_field = field;
            match field {
                HostControlField::ConnectHost
                | HostControlField::ConnectPassword
                | HostControlField::ConnectTotp => {
                    ui.page = HostControlPage::Connect;
                }
                HostControlField::HostPassword => {
                    ui.page = HostControlPage::Host;
                }
                HostControlField::AbName | HostControlField::AbNodeId => {
                    ui.page = HostControlPage::Book;
                }
                HostControlField::WolMac => {
                    ui.page = HostControlPage::Tools;
                }
            }
        }
        self.bump();
    }

    fn handle_keystroke(&self, keystroke: &Keystroke) {
        let page = self
            .ui
            .lock()
            .map(|u| u.page)
            .unwrap_or(HostControlPage::Host);
        if keystroke.key == "enter" || keystroke.key == "return" {
            match page {
                HostControlPage::Connect => self.connect(),
                HostControlPage::Book => self.save_address_book_entry(),
                HostControlPage::Tools => self.send_wol(),
                HostControlPage::Host => self.save_settings(),
                _ => {}
            }
            return;
        }
        let editable = matches!(
            page,
            HostControlPage::Connect
                | HostControlPage::Book
                | HostControlPage::Tools
                | HostControlPage::Host
        );
        if !editable {
            return;
        }
        if let Ok(mut ui) = self.ui.lock() {
            let input = match ui.active_field {
                HostControlField::ConnectHost => &mut ui.connect_host,
                HostControlField::ConnectPassword => &mut ui.connect_password,
                HostControlField::ConnectTotp => &mut ui.connect_totp,
                HostControlField::HostPassword => &mut ui.settings.unattended_password,
                HostControlField::AbName => &mut ui.ab_name,
                HostControlField::AbNodeId => &mut ui.ab_node_id,
                HostControlField::WolMac => &mut ui.wol_mac,
            };
            match keystroke.key.as_str() {
                "backspace" => {
                    input.pop();
                }
                "escape" => {
                    input.clear();
                }
                key if key.len() == 1 => {
                    if let Some(ch) = key.chars().next() {
                        input.push(ch);
                    }
                }
                _ => {}
            }
        }
        self.bump();
    }

    fn clone_for_input(&self) -> RdpHostControlInputHandle {
        RdpHostControlInputHandle {
            ui: self.ui.clone(),
            generation: self.generation.clone(),
            view: self.clone_refs(),
        }
    }
}

#[derive(Clone)]
struct RdpHostControlInputHandle {
    ui: Arc<Mutex<HostControlUi>>,
    generation: Arc<Mutex<u64>>,
    view: RdpHostControlView,
}

impl RdpHostControlInputHandle {
    fn handle_keystroke(&self, keystroke: &Keystroke) {
        self.view.handle_keystroke(keystroke);
        if let Ok(mut g) = self.generation.lock() {
            *g = g.saturating_add(1);
        }
    }
}

fn field_label(field: HostControlField) -> &'static str {
    match field {
        HostControlField::ConnectHost => "Host Node ID",
        HostControlField::ConnectPassword => "密码",
        HostControlField::ConnectTotp => "TOTP",
        HostControlField::HostPassword => "访问密码",
        HostControlField::AbName => "地址簿名称",
        HostControlField::AbNodeId => "地址簿 Node ID",
        HostControlField::WolMac => "WoL MAC",
    }
}

const QUALITY_PRESETS: &[&str] = &["smooth", "balanced", "high", "ultra"];

fn quality_label(preset: &str) -> &str {
    match preset {
        "smooth" => "流畅",
        "balanced" => "平衡",
        "high" => "高清",
        "ultra" => "原画",
        _ => preset,
    }
}

fn next_quality_preset(current: &str) -> String {
    let idx = QUALITY_PRESETS
        .iter()
        .position(|p| *p == current)
        .unwrap_or(0);
    QUALITY_PRESETS[(idx + 1) % QUALITY_PRESETS.len()].to_string()
}

fn next_host_codec(current: &str, supported: &[String]) -> String {
    let mut options = vec!["auto".to_string()];
    options.extend(supported.iter().cloned());
    let idx = options.iter().position(|c| c == current).unwrap_or(0);
    options[(idx + 1) % options.len()].clone()
}

fn format_host_body(ui: &HostControlUi) -> String {
    let password = if ui.settings.unattended_password.is_empty() {
        "（空）".to_string()
    } else {
        "••••".to_string()
    };
    let totp_secret = ui.settings.totp_secret.as_deref().unwrap_or("（未生成）");
    let vram = if ui.vram_available {
        if ui.settings.host_use_vram {
            "开"
        } else {
            "关"
        }
    } else {
        "不可用"
    };
    let logon = if cfg!(windows) {
        if ui.settings.windows_logon_task {
            if ui.logon_task_installed {
                "已启用（已注册）"
            } else {
                "已启用（未注册，保存后生效）"
            }
        } else {
            "关闭"
        }
    } else {
        "仅 Windows"
    };
    let service = if cfg!(windows) {
        if ui.settings.windows_service_mode {
            if ui.windows_service_installed {
                "已启用（WormholeRdpService 已注册）"
            } else {
                "已启用（未注册，保存后生效）"
            }
        } else {
            "关闭"
        }
    } else {
        "仅 Windows"
    };
    format!(
        "本机 Node ID\n{}\n\n无人值守：{}\n启动自监听：{}\n访问密码：{}\n\nFPS：{}  监视器：{}\n画质：{} ({})  编码：{}\nVRAM：{}\n隐私屏：{}\nTOTP：{}  密钥：{}\n登录计划任务：{}\nWindows 服务模式：{}\n\n聚焦「访问密码」后键盘输入；Enter 保存设置。\n当前聚焦：{}\n\n{}",
        ui.node_id,
        if ui.settings.unattended_enabled {
            "已启用"
        } else {
            "关闭"
        },
        if ui.settings.auto_start_on_launch {
            "是"
        } else {
            "否"
        },
        password,
        ui.settings.host_fps,
        ui.settings.host_monitor,
        quality_label(&ui.settings.quality_preset),
        ui.settings.quality_preset,
        ui.settings.host_codec.to_uppercase(),
        vram,
        if ui.settings.privacy_screen {
            "开"
        } else {
            "关"
        },
        if ui.settings.totp_required {
            "必需"
        } else {
            "关"
        },
        totp_secret,
        logon,
        service,
        field_label(ui.active_field),
        ui.status,
    )
}

fn format_discovered(sessions: &[RemoteDesktopSessionDto]) -> String {
    let hosts: Vec<_> = sessions
        .iter()
        .filter(|s| s.discovered && s.role == SessionRole::Host)
        .collect();
    if hosts.is_empty() {
        return String::new();
    }
    let mut lines = vec!["发现的 Host（gossip）".to_string()];
    for host in hosts {
        let peer = if host.peer.len() > 16 {
            format!("{}…", &host.peer[..16])
        } else {
            host.peer.clone()
        };
        lines.push(format!(
            "· {} · {}×{} · {}fps · {peer}",
            host.hostname.as_deref().unwrap_or("Remote Host"),
            host.width,
            host.height,
            host.fps
        ));
    }
    lines.join("\n")
}

fn format_sessions(sessions: &[RemoteDesktopSessionDto]) -> String {
    if sessions.is_empty() {
        return "（无活动会话）".into();
    }
    let mut lines = Vec::new();
    for s in sessions {
        lines.push(format!(
            "· {:?} · {:?} · {} · {}x{} · {}\n  id: {}",
            s.role, s.status, s.peer, s.width, s.height, s.codec, s.id
        ));
    }
    lines.join("\n")
}

impl Entity for RdpHostControlView {
    type Event = ();
}

impl View for RdpHostControlView {
    fn ui_name() -> &'static str {
        "RdpHostControlView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        let _ = self.generation.lock().map(|g| *g).unwrap_or(0);
        let ui = self.ui.lock().ok();
        let Some(ui) = ui else {
            return Container::new(
                ui_text::body("加载中…", self.font)
                    .with_color(ColorU::white())
                    .finish(),
            )
            .finish();
        };

        let page = ui.page;
        let sessions = ui.sessions.clone();
        let book_entries = ui.book_entries.clone();
        let mut tabs = Flex::row();
        for (label, tab) in [
            ("Host", HostControlPage::Host),
            ("Connect", HostControlPage::Connect),
            ("地址簿", HostControlPage::Book),
            ("工具", HostControlPage::Tools),
            ("Sessions", HostControlPage::Sessions),
            ("Audit", HostControlPage::Audit),
        ] {
            let ui_slot = self.ui.clone();
            let gen = self.generation.clone();
            tabs = tabs.with_child(link_label(label, self.font, page == tab, move || {
                if let Ok(mut u) = ui_slot.lock() {
                    u.page = tab;
                }
                if let Ok(mut g) = gen.lock() {
                    *g = g.saturating_add(1);
                }
            }));
        }

        let body = match ui.page {
            HostControlPage::Host => format_host_body(&ui),
            HostControlPage::Connect => {
                let host = if ui.connect_host.is_empty() {
                    "（空）".to_string()
                } else {
                    ui.connect_host.clone()
                };
                let password = if ui.connect_password.is_empty() {
                    "（空）".to_string()
                } else {
                    "••••".to_string()
                };
                let totp = if ui.connect_totp.is_empty() {
                    "（空）".to_string()
                } else {
                    ui.connect_totp.clone()
                };
                let mut text = format!(
                    "Host：{host}\n密码：{password}\nTOTP：{totp}\n\n聚焦字段后键盘输入；Enter 连接；Esc 清空当前字段。\n当前聚焦：{}\n\n{}",
                    field_label(ui.active_field),
                    ui.status,
                );
                if !ui.discovered_text.is_empty() {
                    text.push_str("\n\n");
                    text.push_str(&ui.discovered_text);
                }
                text
            }
            HostControlPage::Book => {
                let name = if ui.ab_name.is_empty() {
                    "（空）".to_string()
                } else {
                    ui.ab_name.clone()
                };
                let node = if ui.ab_node_id.is_empty() {
                    "（空）".to_string()
                } else {
                    ui.ab_node_id.clone()
                };
                format!(
                    "名称：{name}\nNode ID：{node}\n\n{}\n\n聚焦字段后输入；Enter 保存。\n当前聚焦：{}\n\n{}",
                    format_addressbook_entries(&book_entries),
                    field_label(ui.active_field),
                    ui.status,
                )
            }
            HostControlPage::Tools => {
                let mac = if ui.wol_mac.is_empty() {
                    "（空）".to_string()
                } else {
                    ui.wol_mac.clone()
                };
                format!(
                    "高级工具\n────────\n文件传输 / 隧道 / 终端 / 虚拟摄像头在原生 RDP 窗顶栏。\n\nWake-on-LAN\nMAC：{mac}\n\n聚焦 MAC 后输入；Enter 发送魔术包。\n当前聚焦：{}\n\n{}",
                    field_label(ui.active_field),
                    ui.status,
                )
            }
            HostControlPage::Sessions => {
                format!("活动会话\n────────\n{}\n\n{}", ui.sessions_text, ui.status)
            }
            HostControlPage::Audit => format!(
                "连接审计（最近 200 条）\n────────\n{}\n\n{}",
                ui.audit_text, ui.status
            ),
        };

        let mut host_actions = Flex::column();
        if page == HostControlPage::Host {
            let vram_available = ui.vram_available;
            let active_password = ui.active_field == HostControlField::HostPassword;
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "切换无人值守",
                self.font,
                false,
                move || v.toggle_unattended(),
            ));
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "切换启动自监听",
                self.font,
                false,
                move || v.toggle_auto_start(),
            ));
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "编辑访问密码",
                self.font,
                active_password,
                move || {
                    v.focus_field(HostControlField::HostPassword);
                },
            ));
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "保存设置（Enter）",
                self.font,
                false,
                move || {
                    v.save_settings();
                },
            ));
            for (label, delta) in [("FPS −", -5i32), ("FPS +", 5i32)] {
                let v = self.clone_refs();
                host_actions =
                    host_actions.with_child(link_label(label, self.font, false, move || {
                        v.bump_fps(delta);
                    }));
            }
            for (label, delta) in [("监视器 −", -1i32), ("监视器 +", 1i32)] {
                let v = self.clone_refs();
                host_actions =
                    host_actions.with_child(link_label(label, self.font, false, move || {
                        v.bump_monitor(delta);
                    }));
            }
            let v = self.clone_refs();
            host_actions =
                host_actions.with_child(link_label("切换画质", self.font, false, move || {
                    v.cycle_quality();
                }));
            let v = self.clone_refs();
            host_actions =
                host_actions.with_child(link_label("切换编码", self.font, false, move || {
                    v.cycle_codec();
                }));
            if vram_available {
                let v = self.clone_refs();
                host_actions = host_actions.with_child(link_label(
                    "切换 VRAM",
                    self.font,
                    false,
                    move || {
                        v.toggle_vram();
                    },
                ));
            }
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "切换隐私屏",
                self.font,
                false,
                move || {
                    v.toggle_privacy_screen();
                },
            ));
            let v = self.clone_refs();
            host_actions =
                host_actions.with_child(link_label("切换 TOTP", self.font, false, move || {
                    v.toggle_totp_required();
                }));
            let v = self.clone_refs();
            host_actions = host_actions.with_child(link_label(
                "生成 TOTP 密钥",
                self.font,
                false,
                move || {
                    v.generate_totp();
                },
            ));
            #[cfg(windows)]
            {
                let v = self.clone_refs();
                host_actions = host_actions.with_child(link_label(
                    "切换登录计划任务",
                    self.font,
                    false,
                    move || {
                        v.toggle_logon_task();
                    },
                ));
                let v = self.clone_refs();
                host_actions = host_actions.with_child(link_label(
                    "切换 Windows 服务模式",
                    self.font,
                    false,
                    move || {
                        v.toggle_windows_service_mode();
                    },
                ));
            }
            let v = self.clone_refs();
            host_actions =
                host_actions.with_child(link_label("开始共享", self.font, false, move || {
                    v.start_sharing();
                }));
        }

        let mut actions = Flex::row();
        if page == HostControlPage::Host {
            // host controls live in host_actions column
        } else if page == HostControlPage::Connect {
            for (label, field) in [
                ("编辑 Host", HostControlField::ConnectHost),
                ("编辑密码", HostControlField::ConnectPassword),
                ("编辑 TOTP", HostControlField::ConnectTotp),
            ] {
                let v = self.clone_refs();
                let active = ui.active_field == field;
                actions = actions.with_child(link_label(label, self.font, active, move || {
                    v.focus_field(field);
                }));
            }
            let v = self.clone_refs();
            actions = actions.with_child(link_label(
                "连接（Enter）",
                self.font,
                false,
                move || {
                    v.connect();
                },
            ));
        } else if page == HostControlPage::Book {
            for (label, field) in [
                ("编辑名称", HostControlField::AbName),
                ("编辑 Node ID", HostControlField::AbNodeId),
            ] {
                let v = self.clone_refs();
                let active = ui.active_field == field;
                actions = actions.with_child(link_label(label, self.font, active, move || {
                    v.focus_field(field);
                }));
            }
            let v = self.clone_refs();
            actions = actions.with_child(link_label(
                "保存（Enter）",
                self.font,
                false,
                move || {
                    v.save_address_book_entry();
                },
            ));
        } else if page == HostControlPage::Tools {
            let v = self.clone_refs();
            actions = actions.with_child(link_label(
                "编辑 MAC",
                self.font,
                ui.active_field == HostControlField::WolMac,
                move || {
                    v.focus_field(HostControlField::WolMac);
                },
            ));
            let v = self.clone_refs();
            actions = actions.with_child(link_label("发送 WoL", self.font, false, move || {
                v.send_wol();
            }));
        } else if page == HostControlPage::Sessions {
            let v = self.clone_refs();
            actions = actions.with_child(link_label("停止全部", self.font, false, move || {
                v.stop_all();
            }));
        } else if page == HostControlPage::Audit {
            let v = self.clone_refs();
            actions = actions.with_child(link_label("刷新", self.font, false, move || {
                v.refresh_audit_now();
            }));
            let v = self.clone_refs();
            actions = actions.with_child(link_label("修剪日志", self.font, false, move || {
                v.trim_audit_log();
            }));
        }

        let mut session_actions = Flex::column();
        if page == HostControlPage::Sessions {
            for session in &sessions {
                let sid = session.id.clone();
                let short = if sid.len() > 14 {
                    format!("{}…", &sid[..14])
                } else {
                    sid.clone()
                };
                let label = format!("停止 · {short}");
                let v = self.clone_refs();
                session_actions =
                    session_actions.with_child(link_label(&label, self.font, false, move || {
                        v.stop_session(sid.clone());
                    }));
            }
        }

        let mut book_actions = Flex::column();
        if page == HostControlPage::Book {
            for entry in book_entries {
                let peer = entry.node_id.clone();
                let label = format!("连接 · {}", entry.name);
                let v = self.clone_refs();
                book_actions =
                    book_actions.with_child(link_label(&label, self.font, false, move || {
                        v.connect_from_book(peer.clone());
                    }));
            }
        }

        let mut discovered_actions = Flex::column();
        if page == HostControlPage::Connect {
            for host in sessions
                .iter()
                .filter(|s| s.discovered && s.role == SessionRole::Host)
            {
                let peer = host.peer.clone();
                let name = host
                    .hostname
                    .clone()
                    .unwrap_or_else(|| "Remote Host".into());
                let requires_password = host.requires_password;
                let connect_label = format!(
                    "连接 · {}",
                    host.hostname.as_deref().unwrap_or("Remote Host")
                );
                let v = self.clone_refs();
                let peer_connect = peer.clone();
                discovered_actions = discovered_actions.with_child(link_label(
                    &connect_label,
                    self.font,
                    false,
                    move || {
                        v.connect_discovered(peer_connect.clone(), requires_password);
                    },
                ));
                let v = self.clone_refs();
                let peer_book = peer.clone();
                let name_book = name.clone();
                discovered_actions = discovered_actions.with_child(link_label(
                    "加入地址簿",
                    self.font,
                    false,
                    move || {
                        v.add_discovered_to_book(peer_book.clone(), name_book.clone());
                    },
                ));
            }
        }

        let column = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::title("Remote Desktop 控制台", self.font)
                    .with_color(ColorU::new(255, 255, 255, 255))
                    .finish(),
            )
            .with_child(tabs.finish())
            .with_child(
                ui_text::mono(body, self.mono)
                    .with_color(ColorU::new(210, 215, 225, 255))
                    .finish(),
            )
            .with_child(discovered_actions.finish())
            .with_child(host_actions.finish())
            .with_child(book_actions.finish())
            .with_child(session_actions.finish())
            .with_child(actions.finish())
            .finish();

        let root = EventHandler::new(column)
            .with_always_handle()
            .on_keydown({
                let handle = self.clone_for_input();
                move |_, _, keystroke| {
                    handle.handle_keystroke(keystroke);
                    DispatchEventResult::StopPropagation
                }
            })
            .finish();

        Container::new(root)
            .with_uniform_padding(16.)
            .with_background_color(ColorU::new(18, 22, 30, 255))
            .finish()
    }
}

impl RdpHostControlView {
    fn clone_refs(&self) -> Self {
        Self {
            data_dir: self.data_dir.clone(),
            runtime: self.runtime.clone(),
            coordinator: self.coordinator.clone(),
            ui: self.ui.clone(),
            generation: self.generation.clone(),
            font: self.font,
            mono: self.mono,
        }
    }
}

impl Clone for RdpHostControlView {
    fn clone(&self) -> Self {
        self.clone_refs()
    }
}

impl TypedActionView for RdpHostControlView {
    type Action = ();

    fn handle_action(&mut self, _action: &Self::Action, _ctx: &mut ViewContext<Self>) {}
}
