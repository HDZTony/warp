use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::display_commands::{
    display_status, platform_info, DisplayStatusDto, PlatformInfoDto,
};

enum HostStatus {
    Loading,
    Ready(DisplayStatusDto),
    Error(String),
}

pub struct DisplayView {
    core: CoreHandle,
    font: FamilyId,
    platform: PlatformInfoDto,
    host: HostStatus,
}

impl DisplayView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            platform: platform_info(),
            host: HostStatus::Loading,
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.host = HostStatus::Loading;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let ctx = core.runtime().ctx.clone();
                display_status(&ctx.display_state).await
            },
            |view, output, ctx| {
                view.host = match output {
                    Ok(status) => HostStatus::Ready(status),
                    Err(e) => HostStatus::Error(e),
                };
                ctx.notify();
            },
        );
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
        col.add_child(self.capability_row("iPad Display Host", self.platform.supports_display_host));
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
            "推流服务端口与运行状态。完整启动/停止控制即将推出。",
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );
        match &self.host {
            HostStatus::Loading => {
                col.add_child(status_line("正在读取 Host 状态…", self.font, StatusTone::Muted));
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
            }
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
}

impl Entity for DisplayView {
    type Event = ();
}

impl View for DisplayView {
    fn ui_name() -> &'static str {
        "DisplayView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
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

#[cfg(test)]
mod tests {
    use super::DisplayView;

    #[test]
    fn display_view_ui_name() {
        assert_eq!(DisplayView::ui_name(), "DisplayView");
    }
}
