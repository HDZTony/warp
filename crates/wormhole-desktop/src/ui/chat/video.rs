use std::sync::{Arc, Mutex};

use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::chat::shell::ConversationSelection;
use crate::ui::core_handle::CoreHandle;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::chat_rtc_call::{chat_video_call_status, ChatVideoCallConvParams};

pub struct ChatVideoView {
    core: CoreHandle,
    selection: ConversationSelection,
    font: FamilyId,
    status: String,
}

impl ChatVideoView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        selection: ConversationSelection,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let view = Self {
            core,
            selection,
            font,
            status: "视频通话未连接".into(),
        };
        view.start_poll(ctx);
        view
    }

    fn start_poll(&self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            },
            |view, _, ctx| {
                view.refresh_status(ctx);
                view.start_poll(ctx);
            },
        );
    }

    fn refresh_status(&mut self, ctx: &mut ViewContext<Self>) {
        let conv_id = match self.selection.lock().ok().and_then(|g| g.clone()) {
            Some(id) => id,
            None => return,
        };
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let params = ChatVideoCallConvParams { conv_id };
                chat_video_call_status(&runtime.state, params).await
            },
            |view, output, ctx| {
                view.status = match output {
                    Ok(s) => format!(
                        "视频: {} hosting={} peer_live={}",
                        s.phase, s.hosting, s.peer_live
                    ),
                    Err(e) => format!("视频错误: {e}"),
                };
                ctx.notify();
            },
        );
    }
}

impl Entity for ChatVideoView {
    type Event = ();
}

impl View for ChatVideoView {
    fn ui_name() -> &'static str {
        "ChatVideoView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Container::new(
            Flex::column()
                .with_child(ui_text::body("视频通话", self.font).finish())
                .with_child(ui_text::mono(self.status.clone(), self.font).finish())
                .finish(),
        )
        .with_background(theme::accent_bg(16))
        .with_uniform_padding(6.0)
        .finish()
    }
}
