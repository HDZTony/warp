use warpui::elements::ChildView;
use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, UpdateView, View, ViewContext};

use crate::ui::agent_providers_view::AgentProvidersView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::commands::vault_status;

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    data_dir: String,
    vault_line: String,
    sync_panel: warpui::ViewHandle<SyncView>,
    agent_providers: warpui::ViewHandle<AgentProvidersView>,
}

impl SettingsView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        import_model: SharedCodexProviderImportModel,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let data_dir = core.data_dir().display().to_string();
        let sync_panel = ctx.add_view(|ctx| SyncView::new(ctx, core.clone()));
        let agent_providers =
            ctx.add_view(|ctx| AgentProvidersView::new(ctx, core.clone(), import_model));
        let mut view = Self {
            core,
            font,
            data_dir,
            vault_line: "加载 Vault…".into(),
            sync_panel,
            agent_providers,
        };
        view.refresh_vault(ctx);
        view
    }

    pub fn agent_providers_view(&self) -> &warpui::ViewHandle<AgentProvidersView> {
        &self.agent_providers
    }

    pub fn open_deeplink_url(&mut self, url: String, ctx: &mut ViewContext<Self>) {
        let agent = self.agent_providers.clone();
        ctx.update_view(&agent, |view, ctx| view.open_deeplink_url(url, ctx));
    }

    fn refresh_vault(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                vault_status(&state).await
            },
            |view, output, ctx| {
                view.vault_line = match output {
                    Ok(s) => format!(
                        "Vault ready={} endpoint={}",
                        s.ready,
                        s.endpoint_id.as_deref().unwrap_or("—")
                    ),
                    Err(e) => format!("Vault 错误: {e}"),
                };
                ctx.notify();
            },
        );
    }
}

impl Entity for SettingsView {
    type Event = ();
}

impl View for SettingsView {
    fn ui_name() -> &'static str {
        "SettingsView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let col = Flex::column()
            .with_child(ui_text::title("设置", self.font).finish())
            .with_child(ui_text::body(format!("数据目录: {}", self.data_dir), self.font).finish())
            .with_child(ui_text::body(self.vault_line.clone(), self.font).finish())
            .with_child(
                Container::new(ChildView::new(&self.agent_providers).finish())
                    .with_uniform_padding(8.0)
                    .finish(),
            )
            .with_child(
                Container::new(ChildView::new(&self.sync_panel).finish())
                    .with_uniform_padding(8.0)
                    .finish(),
            );
        Container::new(col.finish())
            .with_background(theme::panel())
            .with_uniform_padding(12.0)
            .finish()
    }
}
