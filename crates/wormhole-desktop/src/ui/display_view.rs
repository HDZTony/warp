use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::display_commands::{display_status, platform_info};

pub struct DisplayView {
    core: CoreHandle,
    font: FamilyId,
    summary: String,
}

impl DisplayView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let platform = platform_info();
        let mut view = Self {
            core,
            font,
            summary: format!(
                "平台: {} / Display Host: {} / Mac Client: {}",
                platform.os, platform.supports_display_host, platform.supports_mac_client
            ),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let base = self.summary.clone();
        ctx.spawn(
            async move {
                let ctx = core.runtime().ctx.clone();
                display_status(&ctx.display_state).await
            },
            move |view, output, ctx| {
                let extra = match output {
                    Ok(s) => format!(
                        "\nHost running={} video={} input={} audio={}",
                        s.running, s.video_port, s.input_port, s.audio_port
                    ),
                    Err(e) => format!("\n状态错误: {e}"),
                };
                view.summary = format!("{base}{extra}");
                ctx.notify();
            },
        );
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
        let col = Flex::column()
            .with_child(ui_text::title("虚拟显示器", self.font).finish())
            .with_child(ui_text::mono(self.summary.clone(), self.font).finish());
        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}
