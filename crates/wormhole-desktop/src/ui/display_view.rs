use warpui::elements::{
    Container, DispatchEventResult, EventHandler, Flex, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{
    fetch_device_gate_status, wrap_with_device_gate, DeviceGateStatus, DEVICE_GATE_POLL_INTERVAL,
};
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::display_commands::{
    display_status, platform_info, start_display, stop_display, DisplayStatusDto, PlatformInfoDto,
    StartDisplayParams,
};

enum HostStatus {
    Loading,
    Ready(DisplayStatusDto),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum DisplayAction {
    Refresh,
    StartHost,
    StopHost,
}

pub struct DisplayView {
    core: CoreHandle,
    font: FamilyId,
    gate: DeviceGateStatus,
    platform: PlatformInfoDto,
    host: HostStatus,
    host_busy: bool,
    host_message: String,
    host_tone: StatusTone,
}

impl DisplayView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            gate: DeviceGateStatus::default(),
            platform: platform_info(),
            host: HostStatus::Loading,
            host_busy: false,
            host_message: String::new(),
            host_tone: StatusTone::Placeholder,
        };
        view.refresh(ctx);
        view
    }

    fn schedule_gate_then_refresh(&self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                tokio::time::sleep(DEVICE_GATE_POLL_INTERVAL).await;
                let state = core.runtime().state.clone();
                fetch_device_gate_status(&state).await
            },
            |view, gate, ctx| {
                view.gate = gate.clone();
                if gate.needs_poll() {
                    view.schedule_gate_then_refresh(ctx);
                } else {
                    view.refresh(ctx);
                }
                ctx.notify();
            },
        );
    }

    pub fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.host = HostStatus::Loading;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let gate = fetch_device_gate_status(&state).await;
                let ctx = core.runtime().ctx.clone();
                let host = display_status(&ctx.display_state).await;
                (gate, host)
            },
            |view, output, ctx| {
                let (gate, host) = output;
                view.gate = gate.clone();
                if gate.needs_poll() {
                    view.schedule_gate_then_refresh(ctx);
                }
                view.host = match host {
                    Ok(status) => HostStatus::Ready(status),
                    Err(e) => HostStatus::Error(e),
                };
                ctx.notify();
            },
        );
    }

    fn action_button(&self, label: &str, action: DisplayAction, disabled: bool) -> Box<dyn Element> {
        let label = label.to_string();
        let automation_id = format!("display:btn:{label}");
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if disabled {
                        theme::placeholder()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if !disabled {
                    ctx.dispatch_typed_action(action.clone());
                }
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_cool_bg(if disabled { 8 } else { 24 }))
        .with_border(warpui::elements::Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(6.0),
        ))
        .finish()
    }

    fn capability_row(&self, label: &str, supported: bool) -> Box<dyn Element> {
        let value = if supported { "支持" } else { "不支持" };
        let tone = if supported {
            StatusTone::Success
        } else {
            StatusTone::Muted
        };
        Flex::row()
            .with_child(
                ui_text::body(format!("{label}："), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(status_line(value, self.font, tone))
            .finish()
    }

    fn platform_section(&self) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(section_title("平台能力", self.font));
        col.add_child(section_hint(
            "当前系统可使用的虚拟显示器相关能力（只读状态）。",
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );
        col.add_child(
            ui_text::body(format!("操作系统：{}", self.platform.os), self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            self.capability_row("iPad Display Host", self.platform.supports_display_host),
        );
        col.add_child(self.capability_row("扩展屏 (extend)", self.platform.supports_extend));
        col.add_child(self.capability_row("iPad USB", self.platform.supports_ipad_usb));
        col.add_child(self.capability_row("Mac Client 收流", self.platform.supports_mac_client));
        col.add_child(self.capability_row("系统音频采集", self.platform.supports_system_audio));
        section_card(col.finish())
    }

    fn host_section(&self) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(section_title("Host 状态", self.font));
        col.add_child(section_hint(
            "将本机画面推送到 iPad（mirror/extend）或供其他 Mac Client 收流。",
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );
        match &self.host {
            HostStatus::Loading => {
                col.add_child(status_line(
                    "正在读取 Host 状态…",
                    self.font,
                    StatusTone::Muted,
                ));
            }
            HostStatus::Error(message) => {
                col.add_child(status_line(message.clone(), self.font, StatusTone::Danger));
            }
            HostStatus::Ready(status) => {
                if status.running {
                    col.add_child(status_line("Host 运行中", self.font, StatusTone::Success));
                } else {
                    col.add_child(status_line("Host 未启动", self.font, StatusTone::Neutral));
                }
                let ports = format!(
                    "视频 :{} · 输入 :{} · 音频 :{}",
                    status.video_port, status.input_port, status.audio_port
                );
                col.add_child(
                    ui_text::mono(ports, self.font)
                        .with_color(theme::text())
                        .finish(),
                );
                if let Some(err) = &status.last_error {
                    col.add_child(status_line(
                        format!("最近错误：{err}"),
                        self.font,
                        StatusTone::Warn,
                    ));
                }
                if self.platform.supports_display_host {
                    let mut actions = Flex::row();
                    actions.add_child(self.action_button(
                        if self.host_busy {
                            "处理中…"
                        } else if status.running {
                            "停止 Host"
                        } else {
                            "启动 Host（mirror）"
                        },
                        if status.running {
                            DisplayAction::StopHost
                        } else {
                            DisplayAction::StartHost
                        },
                        self.host_busy
                            || (!status.running && !self.platform.supports_display_host),
                    ));
                    actions.add_child(
                        Container::new(self.action_button(
                            "刷新",
                            DisplayAction::Refresh,
                            self.host_busy,
                        ))
                        .with_margin_left(8.0)
                        .finish(),
                    );
                    col.add_child(
                        Container::new(actions.finish())
                            .with_margin_top(10.0)
                            .finish(),
                    );
                }
            }
        }
        if !self.host_message.is_empty() {
            col.add_child(status_line(
                self.host_message.clone(),
                self.font,
                self.host_tone,
            ));
        }
        section_card(col.finish())
    }

    fn limitations_section(&self) -> Box<dyn Element> {
        if self.platform.limitations.is_empty() {
            return Flex::column().finish();
        }
        let mut col = Flex::column();
        col.add_child(section_title("平台说明", self.font));
        for note in &self.platform.limitations {
            col.add_child(section_hint(note.clone(), self.font));
        }
        section_card(col.finish())
    }

    fn display_body(&self) -> Box<dyn Element> {
        let mut page = Flex::column();
        page.add_child(section_title("虚拟显示器", self.font));
        page.add_child(section_hint(
            "将 PC 或 Mac 画面推送到 iPad，或在 Mac 之间有线/局域网收流。",
            self.font,
        ));
        page.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP)
                .finish(),
        );
        page.add_child(self.platform_section());
        page.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP)
                .finish(),
        );
        page.add_child(self.host_section());
        if !self.platform.limitations.is_empty() {
            page.add_child(
                Container::new(Flex::column().finish())
                    .with_vertical_margin(SECTION_GAP)
                    .finish(),
            );
            page.add_child(self.limitations_section());
        }
        Container::new(page.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}

impl Entity for DisplayView {
    type Event = ();
}

impl View for DisplayView {
    fn ui_name() -> &'static str {
        "DisplayView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        wrap_with_device_gate(self.font, "虚拟显示器", &self.gate, self.display_body())
    }
}

impl TypedActionView for DisplayView {
    type Action = DisplayAction;

    fn handle_action(&mut self, action: &DisplayAction, ctx: &mut ViewContext<Self>) {
        match action {
            DisplayAction::Refresh => self.refresh(ctx),
            DisplayAction::StartHost => {
                if self.host_busy {
                    return;
                }
                self.host_busy = true;
                self.host_message = "正在启动 Display Host…".into();
                self.host_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        start_display(
                            &runtime.ctx.display_state,
                            &runtime.ctx.display_client_state,
                            StartDisplayParams {
                                mode: Some("mirror".into()),
                                fps: Some(60),
                                bitrate: None,
                                wifi: Some(true),
                                host: None,
                                no_audio: None,
                                embed_usbmux: None,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.host_busy = false;
                        match output {
                            Ok(()) => {
                                view.host_message = "Display Host 已启动。".into();
                                view.host_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.host_message = format!("启动失败: {error}");
                                view.host_tone = StatusTone::Danger;
                            }
                        }
                        view.refresh(ctx);
                    },
                );
            }
            DisplayAction::StopHost => {
                if self.host_busy {
                    return;
                }
                self.host_busy = true;
                self.host_message = "正在停止 Display Host…".into();
                self.host_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let runtime = core.runtime();
                        stop_display(&runtime.ctx.display_state).await
                    },
                    |view, output, ctx| {
                        view.host_busy = false;
                        match output {
                            Ok(()) => {
                                view.host_message = "Display Host 已停止。".into();
                                view.host_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.host_message = format!("停止失败: {error}");
                                view.host_tone = StatusTone::Danger;
                            }
                        }
                        view.refresh(ctx);
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayView;
    use warpui::View;

    #[test]
    fn display_view_ui_name() {
        assert_eq!(DisplayView::ui_name(), "DisplayView");
    }
}
