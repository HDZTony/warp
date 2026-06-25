use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{section_hint, section_title, status_line, StatusTone};
use crate::ui_text;
use wormhole_desktop_core::cluster_commands::cluster_status;

pub struct DevicesView {
    core: CoreHandle,
    font: FamilyId,
    summary: String,
}

impl DevicesView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            summary: "加载集群状态…".into(),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status(&state).await
            },
            |view, output, ctx| {
                let joined = match output {
                    Ok(s) => {
                        if s.cluster_id.is_none() {
                            format!(
                                "节点: {}\n集群: 未加入\n\n在「聊天」中添加对等方，或从其他设备邀请本节点。",
                                s.local_node_id
                            )
                        } else {
                            format!(
                                "节点: {}\n集群: {}\n成员: {}\n存储卷: {}",
                                s.local_node_id,
                                s.cluster_id.as_deref().unwrap_or("—"),
                                s.nodes.len(),
                                s.storage_volumes.len()
                            )
                        }
                    }
                    Err(e) => format!("集群错误: {e}"),
                };
                view.summary = joined;
                ctx.notify();
            },
        );
    }
}

impl Entity for DevicesView {
    type Event = ();
}

impl View for DevicesView {
    fn ui_name() -> &'static str {
        "DevicesView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let is_error = self.summary.starts_with("集群错误");
        let col = Flex::column()
            .with_child(section_title("设备 / 集群", self.font))
            .with_child(section_hint(
                "本机 Node ID 用于 P2P 连接与聊天配对。",
                self.font,
            ))
            .with_child(if is_error {
                status_line(self.summary.clone(), self.font, StatusTone::Danger)
            } else {
                ui_text::mono(self.summary.clone(), self.font)
                    .with_color(crate::ui::theme::text())
                    .finish()
            });
        Container::new(col.finish())
            .with_background(crate::ui::theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}
