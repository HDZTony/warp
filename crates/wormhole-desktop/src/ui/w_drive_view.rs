use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::device_gate_view::{
    fetch_device_gate_status, wrap_with_device_gate, DeviceGateStatus, DEVICE_GATE_POLL_INTERVAL,
};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::commands::{list_files, vault_status};

/// Vault / shared-files overview (legacy tab id `w_drive`).
pub struct SharedVaultView {
    core: CoreHandle,
    font: FamilyId,
    gate: DeviceGateStatus,
    status: String,
    files: Vec<String>,
}

impl SharedVaultView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            gate: DeviceGateStatus::default(),
            status: "加载中…".into(),
            files: Vec::new(),
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

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                let state = runtime.state.clone();
                let gate = fetch_device_gate_status(&state).await;
                let status = vault_status(&state).await;
                let files = list_files(&state).await;
                (gate, status, files)
            },
            |view, output, ctx| {
                let (gate, status, files) = output;
                view.gate = gate.clone();
                if gate.needs_poll() {
                    view.schedule_gate_then_refresh(ctx);
                }
                view.status = match status {
                    Ok(s) => format!("Vault ready={}", s.ready),
                    Err(e) => format!("Vault 错误: {e}"),
                };
                view.files = files
                    .unwrap_or_default()
                    .into_iter()
                    .map(|f| format!("{} ({})", f.name, f.id))
                    .collect();
                ctx.notify();
            },
        );
    }

    fn drive_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(ui_text::title("共享 / Vault", self.font).finish());
        col.add_child(ui_text::body(self.status.clone(), self.font).finish());
        for line in &self.files {
            col.add_child(ui_text::mono(line.clone(), self.font).finish());
        }
        if self.files.is_empty() {
            col.add_child(ui_text::body("（暂无文件）", self.font).finish());
        }
        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}

impl Entity for SharedVaultView {
    type Event = ();
}

impl View for SharedVaultView {
    fn ui_name() -> &'static str {
        "SharedVaultView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        wrap_with_device_gate(self.font, "共享 / Vault", &self.gate, self.drive_body())
    }
}
