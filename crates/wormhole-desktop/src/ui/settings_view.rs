use warpui::elements::{
    ChildView, ClippedScrollStateHandle, ClippedScrollable, Container, CrossAxisAlignment, Fill,
    Flex, ParentElement, ScrollbarWidth, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, UpdateView, View, ViewContext};

use crate::ui::agent_providers_view::AgentProvidersView;
use crate::ui::codex_provider_import_model::SharedCodexProviderImportModel;
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, truncate_middle, StatusTone,
    SECTION_GAP,
};
use crate::ui::sync_views::SyncView;
use crate::ui::theme;
use crate::ui::w_drive_settings_view::WDriveSettingsView;
use crate::ui_text;
use wormhole_desktop_core::commands::vault_status;

enum VaultDisplay {
    Loading,
    Ready {
        endpoint: String,
    },
    NotReady,
    Error(String),
}

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    data_dir: String,
    vault: VaultDisplay,
    scroll: ClippedScrollStateHandle,
    sync_panel: warpui::ViewHandle<SyncView>,
    w_drive_settings: warpui::ViewHandle<WDriveSettingsView>,
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
        let w_drive_settings = ctx.add_view(|ctx| WDriveSettingsView::new(ctx, core.clone()));
        let agent_providers =
            ctx.add_view(|ctx| AgentProvidersView::new(ctx, core.clone(), import_model));
        let mut view = Self {
            core,
            font,
            data_dir,
            vault: VaultDisplay::Loading,
            scroll: ClippedScrollStateHandle::new(),
            sync_panel,
            w_drive_settings,
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
        self.vault = VaultDisplay::Loading;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                vault_status(&state).await
            },
            |view, output, ctx| {
                view.vault = match output {
                    Ok(s) if s.ready => VaultDisplay::Ready {
                        endpoint: s.endpoint_id.unwrap_or_else(|| "—".into()),
                    },
                    Ok(_) => VaultDisplay::NotReady,
                    Err(e) => VaultDisplay::Error(e),
                };
                ctx.notify();
            },
        );
    }

    fn vault_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("Vault", self.font));
        col.add_child(section_hint(
            "Iroh P2P 保险库状态。同步与聊天依赖 Vault 就绪。",
            self.font,
        ));
        match &self.vault {
            VaultDisplay::Loading => {
                col.add_child(status_line("正在检查 Vault…", self.font, StatusTone::Placeholder));
            }
            VaultDisplay::Ready { endpoint } => {
                col.add_child(status_line("Vault 已就绪", self.font, StatusTone::Success));
                col.add_child(
                    ui_text::mono(
                        format!("endpoint: {}", truncate_middle(endpoint, 72)),
                        self.font,
                    )
                    .with_color(theme::text())
                    .finish(),
                );
            }
            VaultDisplay::NotReady => {
                col.add_child(status_line(
                    "Vault 尚未就绪，请稍后重试或重启应用。",
                    self.font,
                    StatusTone::Warn,
                ));
            }
            VaultDisplay::Error(message) => {
                col.add_child(status_line(
                    format!("Vault 错误: {}", truncate_middle(message, 160)),
                    self.font,
                    StatusTone::Danger,
                ));
            }
        }
        section_card(col.finish())
    }

    fn system_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("系统", self.font));
        col.add_child(section_hint("本地数据与配置目录。", self.font));
        col.add_child(
            ui_text::mono(
                truncate_middle(&self.data_dir, 96),
                self.font,
            )
            .with_color(theme::text())
            .finish(),
        );
        section_card(col.finish())
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
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(ui_text::title("设置", self.font).with_color(theme::text()).finish());
        col.add_child(section_hint(
            "同步、Agent 供应商与 W 盘数据路径。",
            self.font,
        ));
        col.add_child(self.system_block());
        col.add_child(self.vault_block());
        col.add_child(ChildView::new(&self.agent_providers).finish());
        col.add_child(ChildView::new(&self.w_drive_settings).finish());
        col.add_child(ChildView::new(&self.sync_panel).finish());
        col.add_child(
            ui_text::body(crate::ui::fonts::UI_FONT_ATTRIBUTION, self.font)
                .with_color(theme::placeholder())
                .finish(),
        );

        let scrollable = ClippedScrollable::vertical(
            self.scroll.clone(),
            col.finish(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();

        Container::new(Shrinkable::new(1.0, scrollable).finish())
            .with_background(theme::panel())
            .with_uniform_padding(SECTION_GAP)
            .finish()
    }
}
