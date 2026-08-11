use std::sync::{Arc, Mutex};

use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ChildView, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Fill, Flex,
    Hoverable, MainAxisSize, MouseState, MouseStateHandle, ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{
    AppContext, Element, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle,
};

use crate::ui::agent_panel::sidebar::{load_archived_snapshots, ArchivedSessionSnapshot};
use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::clipboard::write_clipboard_text;
use crate::ui::memory_view::MemoryView;
use crate::ui::activity_view::ActivityView;
use crate::ui::subconscious_view::SubconsciousView;
use crate::ui::tokenjuice_view::TokenJuiceView;
use crate::ui::web_search_view::WebSearchView;
use crate::ui::security_view::SecurityView;
use crate::ui::cron_view::CronView;
use crate::ui::panel_primitives::{section_hint, status_line, tab_content_fill, StatusTone};
use crate::ui::text_field_input::{
    render_field_with_caret, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui::agent_providers_view::AgentProvidersView;
use crate::ui::plugins_view::PluginsView;
use crate::ui::display_view::DisplayView;
use crate::ui::toolbox_view::ToolboxView;
use crate::ui_text;
use wormhole_desktop_core::account_profile_prefs;
use wormhole_desktop_core::account_profile_sync;
use wormhole_desktop_core::cluster_commands::cluster_status_fast;
use wormhole_desktop_core::email_connector_commands::{
    email_connector_disconnect, email_connector_list, email_connector_start_oauth,
    email_connector_test_read, EmailConnectorDto,
};
use wormhole_desktop_core::integrations_commands::{
    composio_authorize, composio_create_trigger, composio_delete_connection,
    composio_disable_trigger, composio_drain_triggers, composio_list_capabilities,
    composio_list_connections, composio_list_trigger_history, composio_list_triggers,
    composio_list_toolkits, composio_sync, get_channel_prefs, get_trigger_notify_prefs,
    mcp_install_entry, mcp_list_installed, mcp_search, mcp_uninstall, set_channel_prefs,
    set_trigger_notify_prefs, skills_install, skills_list_installed, skills_search,
    skills_uninstall, ComposioAuthorizeParams, ComposioConnectionIdParams,
    ComposioTriggerIdParams, McpIdParams, McpSearchParams, McpServerEntry, SetChannelPrefsParams,
    SkillCatalogEntry, TriggerNotifyPrefs,
};

use wormhole_desktop_core::settings_cache_commands::{
    clear_settings_cache, settings_cache_status, ClearSettingsCacheParams, SettingsCacheStatusDto,
};
use wormhole_desktop_core::sync_commands::{
    migrate_shared_storage, shared_storage_info, SharedStorageInfoDto,
};
use wormhole_desktop_core::{
    check_desktop_update, clear_cloud_auth_token, cloud_auth_status, download_desktop_update,
    get_network_relay_status, install_desktop_update, save_network_relay_config,
    desktop_app_version, DesktopUpdateStatusDto, NetworkRelayStatusDto, SaveNetworkRelayParams,
};

use crate::ui::desktop_update::DesktopUpdatePhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    Account,
    Security,
    Email,
    Connections,
    Agent,
    Memory,
    Activity,
    Subconscious,
    TokenJuice,
    WebSearch,
    Cron,
    Cluster,
    Relay,
    SharedPath,
    Cache,
    Archive,
    VirtualMachine,
    RdpHost,
    Display,
    Plugins,
    Theme,
    Language,
    About,
}

/// Local UI row for Composio toolkit catalog (avoids a direct wormhole-integrations dep).
#[derive(Debug, Clone)]
struct ComposioCatalogItem {
    slug: String,
    name: String,
    description: String,
    native: bool,
}

/// Local UI row for an active Composio connection.
#[derive(Debug, Clone)]
struct ComposioConnectionItem {
    id: String,
    toolkit: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct ActiveTriggerItem {
    id: String,
    slug: String,
    toolkit: String,
    state: String,
}

#[derive(Debug, Clone)]
struct McpCatalogItem {
    id: String,
    name: String,
    description: String,
    source: String,
    entry: McpServerEntry,
}

#[derive(Debug, Clone)]
struct SkillCatalogItem {
    id: String,
    name: String,
    description: String,
    entry: SkillCatalogEntry,
}

impl SettingsPage {
    fn all() -> &'static [SettingsPage] {
        &[
            SettingsPage::Account,
            SettingsPage::Security,
            SettingsPage::Email,
            SettingsPage::Connections,
            SettingsPage::Agent,
            SettingsPage::Theme,
            SettingsPage::Memory,
            SettingsPage::Activity,
            SettingsPage::Subconscious,
            SettingsPage::TokenJuice,
            SettingsPage::WebSearch,
            SettingsPage::Cron,
            SettingsPage::Cluster,
            SettingsPage::Relay,
            SettingsPage::SharedPath,
            SettingsPage::Cache,
            SettingsPage::Archive,
            SettingsPage::VirtualMachine,
            SettingsPage::RdpHost,
            SettingsPage::Display,
            SettingsPage::Plugins,
            SettingsPage::Language,
            SettingsPage::About,
        ]
    }

    fn nav_automation_id(self) -> &'static str {
        match self {
            SettingsPage::Account => "settings:nav_account",
            SettingsPage::Security => "settings:nav_security",
            SettingsPage::Email => "settings:nav_email",
            SettingsPage::Connections => "settings:nav_connections",
            SettingsPage::Agent => "settings:nav_agent",
            SettingsPage::Memory => "settings:nav_memory",
            SettingsPage::Activity => "settings:nav_activity",
            SettingsPage::Subconscious => "settings:nav_subconscious",
            SettingsPage::TokenJuice => "settings:nav_tokenjuice",
            SettingsPage::WebSearch => "settings:nav_web_search",
            SettingsPage::Cron => "settings:nav_cron",
            SettingsPage::Cluster => "settings:nav_cluster",
            SettingsPage::Relay => "settings:nav_relay",
            SettingsPage::SharedPath => "settings:nav_shared_path",
            SettingsPage::Cache => "settings:nav_cache",
            SettingsPage::Archive => "settings:nav_archive",
            SettingsPage::VirtualMachine => "settings:nav_vm",
            SettingsPage::RdpHost => "settings:nav_rdp_host",
            SettingsPage::Display => "settings:nav_display",
            SettingsPage::Plugins => "settings:nav_plugins",
            SettingsPage::Theme => "settings:nav_theme",
            SettingsPage::Language => "settings:nav_language",
            SettingsPage::About => "settings:nav_about",
        }
    }

    fn title(self) -> String {
        let key = match self {
            SettingsPage::Account => "settings.page.account",
            SettingsPage::Security => "settings.page.security",
            SettingsPage::Email => "settings.page.email",
            SettingsPage::Connections => "settings.page.connections",
            SettingsPage::Agent => "settings.page.agent",
            SettingsPage::Memory => "settings.page.memory",
            SettingsPage::Activity => "settings.page.activity",
            SettingsPage::Subconscious => "settings.page.subconscious",
            SettingsPage::TokenJuice => "settings.page.tokenjuice",
            SettingsPage::WebSearch => "settings.page.web_search",
            SettingsPage::Cron => "settings.page.cron",
            SettingsPage::Cluster => "settings.page.cluster",
            SettingsPage::Relay => "settings.page.relay",
            SettingsPage::SharedPath => "settings.page.shared_path",
            SettingsPage::Cache => "settings.page.cache",
            SettingsPage::Archive => "settings.page.archive",
            SettingsPage::VirtualMachine => "settings.page.vm",
            SettingsPage::RdpHost => "settings.page.rdp_host",
            SettingsPage::Display => "settings.page.display",
            SettingsPage::Plugins => "settings.page.plugins",
            SettingsPage::Theme => "settings.page.theme",
            SettingsPage::Language => "settings.page.language",
            SettingsPage::About => "settings.page.about",
        };
        wormhole_i18n::t(key)
    }

    fn eyebrow(self) -> (&'static str, String) {
        match self {
            SettingsPage::Account => ("ACCOUNT", wormhole_i18n::t("settings.eyebrow.account")),
            SettingsPage::Security => ("SECURITY", wormhole_i18n::t("settings.eyebrow.security")),
            SettingsPage::Email => ("CONNECTORS", wormhole_i18n::t("settings.eyebrow.email")),
            SettingsPage::Connections => {
                ("CONNECTIONS", wormhole_i18n::t("settings.eyebrow.connections"))
            }
            SettingsPage::Agent => ("AGENT", wormhole_i18n::t("settings.eyebrow.agent")),
            SettingsPage::Memory => ("MEMORY", wormhole_i18n::t("settings.eyebrow.memory")),
            SettingsPage::Activity => ("ACTIVITY", wormhole_i18n::t("settings.eyebrow.activity")),
            SettingsPage::Subconscious => (
                "SUBCONSCIOUS",
                wormhole_i18n::t("settings.eyebrow.subconscious"),
            ),
            SettingsPage::TokenJuice => {
                ("TOKENJUICE", wormhole_i18n::t("settings.eyebrow.tokenjuice"))
            }
            SettingsPage::WebSearch => {
                ("WEB SEARCH", wormhole_i18n::t("settings.eyebrow.web_search"))
            }
            SettingsPage::Cron => ("CRON", wormhole_i18n::t("settings.eyebrow.cron")),
            SettingsPage::Cluster => ("CLUSTER", wormhole_i18n::t("settings.eyebrow.cluster")),
            SettingsPage::Relay => ("P2P", wormhole_i18n::t("settings.eyebrow.relay")),
            SettingsPage::SharedPath => ("DATA", wormhole_i18n::t("settings.eyebrow.shared_path")),
            SettingsPage::Cache => ("CACHE", wormhole_i18n::t("settings.eyebrow.cache")),
            SettingsPage::Archive => ("ARCHIVE", wormhole_i18n::t("settings.eyebrow.archive")),
            SettingsPage::VirtualMachine => ("SYSTEM", wormhole_i18n::t("settings.eyebrow.vm")),
            SettingsPage::RdpHost => ("RDP", wormhole_i18n::t("settings.eyebrow.rdp_host")),
            SettingsPage::Display => ("DISPLAY", wormhole_i18n::t("settings.eyebrow.display")),
            SettingsPage::Plugins => ("PLUGIN", wormhole_i18n::t("settings.eyebrow.plugins")),
            SettingsPage::Theme => ("THEME", wormhole_i18n::t("settings.eyebrow.theme")),
            SettingsPage::Language => ("LANG", wormhole_i18n::t("settings.eyebrow.language")),
            SettingsPage::About => ("APP", wormhole_i18n::t("settings.eyebrow.about")),
        }
    }

    fn group_key(self) -> &'static str {
        match self {
            SettingsPage::Account
            | SettingsPage::Security
            | SettingsPage::Email
            | SettingsPage::Connections
            | SettingsPage::Agent
            | SettingsPage::Theme
            | SettingsPage::Memory
            | SettingsPage::Activity
            | SettingsPage::Subconscious
            | SettingsPage::TokenJuice
            | SettingsPage::WebSearch
            | SettingsPage::Cron => "settings.nav.group.general",
            SettingsPage::Cluster | SettingsPage::Relay => "settings.nav.group.cluster",
            SettingsPage::SharedPath => "settings.nav.group.data",
            SettingsPage::Cache | SettingsPage::Archive => "settings.nav.group.storage",
            SettingsPage::VirtualMachine
            | SettingsPage::RdpHost
            | SettingsPage::Display
            | SettingsPage::Plugins
            | SettingsPage::Language
            | SettingsPage::About => "settings.nav.group.system",
        }
    }

    fn group(self) -> String {
        wormhole_i18n::t(self.group_key())
    }

    fn icon_path(self) -> &'static str {
        match self {
            SettingsPage::Account | SettingsPage::Security | SettingsPage::Email => "agent-user.svg",
            SettingsPage::Connections => "tab-toolbox.svg",
            SettingsPage::Agent => "tab-agent.svg",
            SettingsPage::Memory => "share-file.svg",
            SettingsPage::Activity => "share-sync.svg",
            SettingsPage::Subconscious => "share-sync.svg",
            SettingsPage::TokenJuice => "share-sync.svg",
            SettingsPage::WebSearch => "tab-toolbox.svg",
            SettingsPage::Cron => "share-sync.svg",
            SettingsPage::Cluster => "tab-devices.svg",
            SettingsPage::Relay => "cluster-refresh.svg",
            SettingsPage::SharedPath => "share-file.svg",
            SettingsPage::Cache => "share-sync.svg",
            SettingsPage::Archive => "agent-menu-archive.svg",
            SettingsPage::VirtualMachine => "device-pc.svg",
            SettingsPage::RdpHost => "device-pc.svg",
            SettingsPage::Display => "tab-devices.svg",
            SettingsPage::Plugins => "tab-toolbox.svg",
            SettingsPage::Theme => "cluster-refresh.svg",
            SettingsPage::Language => "cluster-refresh.svg",
            SettingsPage::About => "cluster-refresh.svg",
        }
    }

    fn search_haystack(self) -> String {
        let (eyebrow, sub) = self.eyebrow();
        let mut haystack = format!("{} {} {} {}", self.group(), self.title(), eyebrow, sub);
        if self == SettingsPage::Email {
            haystack.push_str(" email gmail outlook");
        }
        if self == SettingsPage::Connections {
            haystack.push_str(
                " composio gmail github notion mcp skills triggers channels telegram discord 连接 集成",
            );
        }
        if self == SettingsPage::Agent {
            haystack.push_str(
                " codex llm api key byok kimi zai deepseek openai anthropic qwen minimax 供应商 provider",
            );
        }
        if self == SettingsPage::Memory {
            haystack.push_str(
                " memory wiki obsidian vault tinycortex 记忆 笔记 reindex sources",
            );
        }
        if self == SettingsPage::Activity {
            haystack.push_str(
                " activity notifications alerts routines automations background subconscious cron triage gmail outlook 通知 活动 定时",
            );
        }
        if self == SettingsPage::Subconscious {
            haystack.push_str(
                " subconscious heartbeat dream reflect memory tick 潜意识 心跳 run now",
            );
        }
        if self == SettingsPage::TokenJuice {
            haystack.push_str(
                " tokenjuice compression ccr cache savings tokens cost 压缩 节省 token",
            );
        }
        if self == SettingsPage::WebSearch {
            haystack.push_str(
                " web search brave exa byok fetch scraper 网页 搜索 agent-web-search",
            );
        }
        if self == SettingsPage::Cron {
            haystack.push_str(
                " cron schedule agent job timer 定时 任务 expression prompt",
            );
        }
        if self == SettingsPage::VirtualMachine {
            haystack.push_str(" 工具箱 toolbox runner 用户程序 上传 指定人 allowlist");
        }
        if self == SettingsPage::RdpHost {
            haystack.push_str(" rdp host 无人值守 隐私屏 totp fps");
        }
        if self == SettingsPage::Display {
            haystack.push_str(" ipad display mirror extend 虚拟显示器");
        }
        if self == SettingsPage::Plugins {
            haystack.push_str(
                " bb-browser chromium browser mcp plugin 插件 插件市场 marketplace codex",
            );
        }
        if self == SettingsPage::Theme {
            haystack.push_str(
                " theme studio appearance classic ocean sepia matrix hal 主题 外观 配色",
            );
        }
        if self == SettingsPage::Language {
            haystack.push_str(" language locale 语言 中文 english zh en i18n");
        }
        if self == SettingsPage::About {
            haystack.push_str(" update version 更新 检查更新 版本");
        }
        haystack
    }

    fn matches_query(self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.search_haystack().to_lowercase().contains(&q)
    }
}

/// Ordered unique group keys for the settings sidebar.
fn settings_nav_group_keys() -> &'static [&'static str] {
    &[
        "settings.nav.group.general",
        "settings.nav.group.cluster",
        "settings.nav.group.data",
        "settings.nav.group.storage",
        "settings.nav.group.system",
    ]
}

fn settings_pages_in_group_key(group_key: &str) -> Vec<SettingsPage> {
    SettingsPage::all()
        .iter()
        .copied()
        .filter(|page| page.group_key() == group_key)
        .collect()
}

#[derive(Debug, Clone)]
pub enum SettingsEvent {
    AccountChanged { authenticated: bool },
    OpenLogin,
    OpenPurchase,
    OpenRedeem,
    OpenClusterManagement,
    OpenRdpHostControl,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
    /// Shared auto-update session changed (banner should refresh).
    UpdateSessionChanged,
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    BrowseMigrate,
    SaveMigration,
    FocusPath,
    FocusSearch,
    TextFieldEdit(TextFieldEditAction),
    SearchFieldEdit(TextFieldEditAction),
    SelectPage(SettingsPage),
    Refresh,
    Login,
    Logout,
    OpenPurchase,
    OpenRedeem,
    RefreshAccount,
    FocusAccountDisplayName,
    AccountDisplayNameEdit(TextFieldEditAction),
    SaveAccountDisplayName,
    RefreshEmailConnectors,
    ConnectEmail(String),
    DisconnectEmail(String),
    TestEmail(String),
    RefreshConnections,
    SelectConnectionsTab(u8),
    ConnectComposio(String),
    DisconnectComposio(String),
    ToggleTriggerNotifyAgent,
    DrainTriggers,
    SyncIntegrations,
    DisableTrigger(String),
    CreateTriggerForConnection(String),
    BrowseMcpCatalog,
    InstallMcp(usize),
    UninstallMcp(String),
    BrowseSkillsCatalog,
    InstallSkill(usize),
    UninstallSkill(String),
    SetDefaultChannel(String),
    ToggleClusterSection,
    OpenClusterManagement,
    ToggleRelaySection,
    ToggleCacheSection,
    ToggleArchiveSection,
    RestoreArchivedSession(String),
    DeleteArchivedSession(String),
    RefreshRelay,
    SelectRelay(String),
    ApplyRelay,
    RefreshCache,
    ClearCache(String),
    CheckDesktopUpdate,
    InstallDesktopUpdate,
    RetryDesktopUpdateDownload,
    OpenRdpHostControl,
    CopyUserId,
    CopyDeviceId,
    /// Persist UI language (`system` / `zh-CN` / `en`) and refresh locale.
    SetUiLanguage(String),
    /// Theme Studio actions (colour / font / backdrop / import-export).
    ThemeStudio(crate::ui::theme_studio::ThemeStudioAction),
}

pub struct SettingsView {
    core: CoreHandle,
    font: FamilyId,
    selected_page: SettingsPage,
    search_query: String,
    search_field: TextFieldState,
    search_focused: bool,
    storage: Option<SharedStorageInfoDto>,
    storage_draft: String,
    storage_field: TextFieldState,
    storage_focused: bool,
    status: String,
    status_tone: StatusTone,
    busy: bool,
    auth_user_id: Option<String>,
    auth_status: String,
    auth_status_tone: StatusTone,
    auth_busy: bool,
    auth_device_id: Option<String>,
    account_display_name: String,
    account_display_name_field: TextFieldState,
    account_display_name_focused: bool,
    account_display_name_busy: bool,
    email_connectors: Vec<EmailConnectorDto>,
    email_busy: bool,
    email_message: String,
    email_tone: StatusTone,
    connections_tab: u8,
    composio_catalog: Vec<ComposioCatalogItem>,
    composio_connections: Vec<ComposioConnectionItem>,
    connections_busy: bool,
    connections_message: String,
    connections_tone: StatusTone,
    trigger_notify_agent: bool,
    trigger_history_preview: String,
    active_triggers: Vec<ActiveTriggerItem>,
    mcp_installed_preview: String,
    skills_installed_preview: String,
    mcp_catalog: Vec<McpCatalogItem>,
    skills_catalog: Vec<SkillCatalogItem>,
    channel_default: String,
    cluster_expanded: bool,
    cluster_id: Option<String>,
    cluster_name: Option<String>,
    cluster_message: String,
    archive_expanded: bool,
    relay_expanded: bool,
    relay_status: Option<NetworkRelayStatusDto>,
    relay_mode: String,
    relay_message: String,
    relay_tone: StatusTone,
    relay_busy: bool,
    cache_expanded: bool,
    cache_status: Option<SettingsCacheStatusDto>,
    cache_message: String,
    cache_tone: StatusTone,
    cache_busy: bool,
    update_status: Option<DesktopUpdateStatusDto>,
    update_message: String,
    update_tone: StatusTone,
    update_busy: bool,
    /// Compile-time app version (always shown on About; does not require a network check).
    app_version: String,
    scroll: ClippedScrollStateHandle,
    /// Settings left-nav list (independent of the right-hand detail pane scroll).
    nav_scroll: ClippedScrollStateHandle,
    toolbox: ViewHandle<ToolboxView>,
    display: ViewHandle<DisplayView>,
    agent_providers: ViewHandle<AgentProvidersView>,
    plugins: ViewHandle<PluginsView>,
    memory: ViewHandle<MemoryView>,
    activity: ViewHandle<ActivityView>,
    subconscious: ViewHandle<SubconsciousView>,
    tokenjuice: ViewHandle<TokenJuiceView>,
    web_search: ViewHandle<WebSearchView>,
    security: ViewHandle<SecurityView>,
    cron: ViewHandle<CronView>,
    theme_studio: crate::ui::theme_studio::ThemeStudioState,
}

impl SettingsView {
    fn archive_expanded_path(core: &CoreHandle) -> std::path::PathBuf {
        core.data_dir().join("settings-archive-expanded")
    }

    fn load_archive_expanded(core: &CoreHandle) -> bool {
        std::fs::read_to_string(Self::archive_expanded_path(core))
            .map(|value| value.trim() == "1")
            .unwrap_or(false)
    }

    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font_themed(ctx);
        let archive_expanded = Self::load_archive_expanded(&core);
        let toolbox =
            ctx.add_typed_action_view(|ctx| ToolboxView::new(ctx, core.clone()));
        let display = ctx.add_typed_action_view(|ctx| DisplayView::new(ctx, core.clone()));
        let agent_providers =
            ctx.add_typed_action_view(|ctx| AgentProvidersView::new(ctx, core.clone()));
        let plugins = ctx.add_typed_action_view(|ctx| PluginsView::new(ctx, core.clone()));
        let memory = ctx.add_typed_action_view(|ctx| MemoryView::new(ctx, core.clone()));
        let activity = ctx.add_typed_action_view(|ctx| ActivityView::new(ctx, core.clone()));
        let subconscious =
            ctx.add_typed_action_view(|ctx| SubconsciousView::new(ctx, core.clone()));
        let tokenjuice =
            ctx.add_typed_action_view(|ctx| TokenJuiceView::new(ctx, core.clone()));
        let web_search =
            ctx.add_typed_action_view(|ctx| WebSearchView::new(ctx, core.clone()));
        let security =
            ctx.add_typed_action_view(|ctx| SecurityView::new(ctx, core.clone()));
        let cron = ctx.add_typed_action_view(|ctx| CronView::new(ctx, core.clone()));
        let mut view = Self {
            core,
            font,
            selected_page: SettingsPage::Account,
            search_query: String::new(),
            search_field: TextFieldState::new(),
            search_focused: false,
            storage: None,
            storage_draft: String::new(),
            storage_field: TextFieldState::new(),
            storage_focused: false,
            status: String::new(),
            status_tone: StatusTone::Placeholder,
            busy: false,
            auth_user_id: None,
            auth_status: String::new(),
            auth_status_tone: StatusTone::Placeholder,
            auth_busy: false,
            auth_device_id: None,
            account_display_name: String::new(),
            account_display_name_field: TextFieldState::new(),
            account_display_name_focused: false,
            account_display_name_busy: false,
            email_connectors: Vec::new(),
            email_busy: false,
            email_message: String::new(),
            email_tone: StatusTone::Placeholder,
            connections_tab: 0,
            composio_catalog: Vec::new(),
            composio_connections: Vec::new(),
            connections_busy: false,
            connections_message: String::new(),
            connections_tone: StatusTone::Placeholder,
            trigger_notify_agent: false,
            trigger_history_preview: String::new(),
            active_triggers: Vec::new(),
            mcp_installed_preview: String::new(),
            skills_installed_preview: String::new(),
            mcp_catalog: Vec::new(),
            skills_catalog: Vec::new(),
            channel_default: "web".into(),
            cluster_expanded: false,
            cluster_id: None,
            cluster_name: None,
            cluster_message: String::new(),
            archive_expanded,
            relay_expanded: false,
            relay_status: None,
            relay_mode: "auto".into(),
            relay_message: String::new(),
            relay_tone: StatusTone::Placeholder,
            relay_busy: false,
            cache_expanded: false,
            cache_status: None,
            cache_message: String::new(),
            cache_tone: StatusTone::Placeholder,
            cache_busy: false,
            update_status: None,
            update_message: String::new(),
            update_tone: StatusTone::Placeholder,
            update_busy: false,
            app_version: desktop_app_version().to_string(),
            scroll: ClippedScrollStateHandle::new(),
            nav_scroll: ClippedScrollStateHandle::new(),
            toolbox,
            display,
            agent_providers,
            plugins,
            memory,
            activity,
            subconscious,
            tokenjuice,
            web_search,
            security,
            cron,
            theme_studio: crate::ui::theme_studio::ThemeStudioState::default(),
        };
        view.refresh(ctx);
        view.refresh_account(ctx);
        view.refresh_email_connectors(ctx);
        view.refresh_connections(ctx);
        view.refresh_cluster(ctx);
        view.refresh_relay(ctx);
        view.refresh_cache(ctx);
        view
    }

    /// Open the Virtual Machine settings page (embedded toolbox).
    pub fn select_virtual_machine(&mut self, ctx: &mut ViewContext<Self>) {
        self.selected_page = SettingsPage::VirtualMachine;
        self.search_focused = false;
        self.storage_focused = false;
        ctx.notify();
    }

    /// Open the Memory settings page (Agent composer chip / deep link).
    pub fn select_memory(&mut self, ctx: &mut ViewContext<Self>) {
        self.selected_page = SettingsPage::Memory;
        self.search_focused = false;
        self.storage_focused = false;
        ctx.notify();
    }

    /// Refresh About update labels from the shared shell/settings update session.
    pub fn refresh_update_ui(&mut self, ctx: &mut ViewContext<Self>) {
        self.sync_update_ui_from_shared();
        ctx.notify();
    }

    /// Start download using the shared update session (also used by the shell banner).
    pub fn start_update_download(&mut self, ctx: &mut ViewContext<Self>) {
        self.begin_update_download(ctx);
    }

    /// Confirm install using the shared update session (also used by the shell banner).
    pub fn start_update_install(&mut self, ctx: &mut ViewContext<Self>) {
        self.begin_update_install(ctx);
    }

    /// Kick a background check (+ auto-download) from the shell poller.
    pub fn start_update_check(&mut self, ctx: &mut ViewContext<Self>) {
        self.begin_update_check(ctx, true);
    }

    pub fn reload_security(&mut self, ctx: &mut ViewContext<Self>) {
        let security = self.security.clone();
        ctx.update_view(&security, |view, ctx| {
            view.reload(ctx);
        });
    }

    pub fn refresh_account(&mut self, ctx: &mut ViewContext<Self>) {
        self.auth_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = cloud_auth_status(&state).await?;
                let account_id = status.user_id.clone().unwrap_or_default();
                let profile = if status.authenticated && !account_id.trim().is_empty() {
                    account_profile_sync::sync_account_profile(&state, &account_id).await?
                } else {
                    account_profile_prefs::AccountProfileRecord::default()
                };
                Ok::<_, String>((status, profile))
            },
            |view, output, ctx| {
                view.auth_busy = false;
                match output {
                    Ok((status, profile)) => {
                        view.auth_user_id = status.user_id;
                        view.auth_device_id = status.device_id;
                        view.account_display_name = profile.display_name;
                        view.account_display_name_field = TextFieldState::new();
                        if status.authenticated && view.auth_device_id.is_some() {
                            view.auth_status = "已登录".into();
                            view.auth_status_tone = StatusTone::Success;
                        } else if status.authenticated {
                            view.auth_status = "已登录，正在恢复设备身份…".into();
                            view.auth_status_tone = StatusTone::Placeholder;
                        } else {
                            view.auth_status = "未登录 — P2P / 集群 / 聊天需先登录。".into();
                            view.auth_status_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(err) => {
                        view.auth_status = format!("读取登录状态失败: {err}");
                        view.auth_status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn edit_account_display_name(
        &mut self,
        edit: &TextFieldEditAction,
        ctx: &mut ViewContext<Self>,
    ) {
        self.account_display_name_field
            .apply(&mut self.account_display_name, edit);
        self.account_display_name_focused = true;
        ctx.notify();
    }

    fn save_account_display_name(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(user_id) = self.auth_user_id.clone() else {
            self.auth_status = wormhole_i18n::t("settings.auth.login_required_for_name");
            self.auth_status_tone = StatusTone::Warn;
            ctx.notify();
            return;
        };
        if self.account_display_name_busy {
            return;
        }
        self.account_display_name_busy = true;
        self.auth_status = wormhole_i18n::t("settings.auth.saving_display_name");
        self.auth_status_tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let display_name = self.account_display_name.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                account_profile_sync::save_account_display_name(&state, &user_id, &display_name)
                    .await
            },
            |view, output, ctx| {
                view.account_display_name_busy = false;
                match output {
                    Ok(record) => {
                        view.account_display_name = record.display_name;
                        view.auth_status = wormhole_i18n::t("settings.auth.display_name_saved");
                        view.auth_status_tone = StatusTone::Success;
                    }
                    Err(err) => {
                        view.auth_status = err;
                        view.auth_status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_email_connectors(&mut self, ctx: &mut ViewContext<Self>) {
        if self.auth_user_id.is_none() {
            self.email_connectors.clear();
            self.email_message = "登录 Wormhole 后可连接邮箱。".into();
            self.email_tone = StatusTone::Placeholder;
            return;
        }
        self.email_busy = true;
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                email_connector_list(&state).await
            },
            |view, output, ctx| {
                view.email_busy = false;
                match output {
                    Ok(result) => {
                        view.email_connectors = result.items;
                        view.email_message.clear();
                        view.email_tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.email_message = format!("读取邮箱连接器失败: {error}");
                        view.email_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_connections(&mut self, ctx: &mut ViewContext<Self>) {
        self.connections_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let toolkits = composio_list_toolkits(&state).await;
                let connections = composio_list_connections(&state).await;
                let caps = composio_list_capabilities();
                let prefs = get_trigger_notify_prefs(&state).unwrap_or_default();
                let history = composio_list_trigger_history(&state, Some(12));
                let triggers = composio_list_triggers(&state).await;
                let mcp = mcp_list_installed(&state);
                let skills = skills_list_installed(&state);
                let mcp_browse = mcp_search(McpSearchParams {
                    query: String::new(),
                    limit: Some(12),
                })
                .await;
                let skills_browse = skills_search(
                    &state,
                    McpSearchParams {
                        query: String::new(),
                        limit: Some(12),
                    },
                )
                .await;
                let channels = get_channel_prefs(&state);
                let _ = wormhole_desktop_core::integrations_commands::refresh_prompt_fragment_cache(
                    &state,
                )
                .await;
                (
                    toolkits,
                    connections,
                    caps,
                    prefs,
                    history,
                    triggers,
                    mcp,
                    skills,
                    mcp_browse,
                    skills_browse,
                    channels,
                )
            },
            |view, output, ctx| {
                view.connections_busy = false;
                let (
                    toolkits,
                    connections,
                    caps,
                    prefs,
                    history,
                    triggers,
                    mcp,
                    skills,
                    mcp_browse,
                    skills_browse,
                    channels,
                ) = output;
                view.trigger_notify_agent = prefs.notify_agent;
                view.channel_default = channels
                    .as_ref()
                    .map(|c| c.default_channel.as_str().to_string())
                    .unwrap_or_else(|_| "web".into());

                let native_slugs: std::collections::HashSet<String> = caps
                    .capabilities
                    .iter()
                    .filter(|c| c.native_provider)
                    .map(|c| c.toolkit.to_ascii_lowercase())
                    .collect();

                let mut catalog = Vec::new();
                let mut catalog_errors = Vec::new();
                match toolkits {
                    Ok(resp) if !resp.catalog.is_empty() => {
                        catalog = resp
                            .catalog
                            .into_iter()
                            .map(|entry| {
                                let name = if entry.name.is_empty() {
                                    entry.slug.clone()
                                } else {
                                    entry.name
                                };
                                let native = native_slugs.contains(&entry.slug.to_ascii_lowercase());
                                ComposioCatalogItem {
                                    slug: entry.slug,
                                    name,
                                    description: entry.description.unwrap_or_default(),
                                    native,
                                }
                            })
                            .collect();
                    }
                    Ok(resp) if !resp.toolkits.is_empty() => {
                        catalog = resp
                            .toolkits
                            .into_iter()
                            .map(|slug| {
                                let native = native_slugs.contains(&slug.to_ascii_lowercase());
                                ComposioCatalogItem {
                                    name: slug.clone(),
                                    slug,
                                    description: String::new(),
                                    native,
                                }
                            })
                            .collect();
                    }
                    Ok(_) => {}
                    Err(error) => catalog_errors.push(format!("toolkits: {error}")),
                }
                if catalog.is_empty() {
                    catalog = caps
                        .capabilities
                        .iter()
                        .map(|cap| ComposioCatalogItem {
                            slug: cap.toolkit.clone(),
                            name: cap.toolkit.clone(),
                            description: cap.description.clone(),
                            native: cap.native_provider,
                        })
                        .collect();
                }
                view.composio_catalog = catalog;

                match connections {
                    Ok(resp) => {
                        view.composio_connections = resp
                            .connections
                            .into_iter()
                            .map(|conn| {
                                let detail = conn
                                    .account_email
                                    .or(conn.username)
                                    .or(conn.workspace)
                                    .unwrap_or_else(|| conn.status.clone());
                                ComposioConnectionItem {
                                    id: conn.id,
                                    toolkit: conn.toolkit,
                                    status: conn.status,
                                    detail,
                                }
                            })
                            .collect();
                    }
                    Err(error) => {
                        view.composio_connections.clear();
                        catalog_errors.push(format!("connections: {error}"));
                    }
                }

                match history {
                    Ok(result) => {
                        if result.entries.is_empty() {
                            view.trigger_history_preview =
                                wormhole_i18n::t("settings.connections.triggers.empty");
                        } else {
                            view.trigger_history_preview = result
                                .entries
                                .iter()
                                .take(8)
                                .map(|entry| {
                                    format!(
                                        "{} · {} · {}",
                                        entry.archived_at, entry.toolkit, entry.trigger_slug
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                        }
                    }
                    Err(error) => {
                        view.trigger_history_preview = format!("history: {error}");
                    }
                }

                match triggers {
                    Ok(resp) => {
                        view.active_triggers = resp
                            .get("triggers")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|item| {
                                let id = item.get("id")?.as_str()?.to_string();
                                if id.is_empty() {
                                    return None;
                                }
                                Some(ActiveTriggerItem {
                                    id,
                                    slug: item
                                        .get("slug")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    toolkit: item
                                        .get("toolkit")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    state: item
                                        .get("state")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("active")
                                        .to_string(),
                                })
                            })
                            .collect();
                    }
                    Err(error) => {
                        view.active_triggers.clear();
                        catalog_errors.push(format!("triggers: {error}"));
                    }
                }

                match mcp {
                    Ok(servers) if servers.is_empty() => {
                        view.mcp_installed_preview =
                            wormhole_i18n::t("settings.connections.mcp.empty");
                    }
                    Ok(servers) => {
                        view.mcp_installed_preview = format!(
                            "{} ({})\n{}",
                            wormhole_i18n::t("settings.connections.mcp.installed"),
                            servers.len(),
                            servers
                                .iter()
                                .map(|s| format!("• {} [{}]", s.name, s.id))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                    }
                    Err(error) => {
                        view.mcp_installed_preview = format!("mcp: {error}");
                    }
                }

                match mcp_browse {
                    Ok(result) => {
                        view.mcp_catalog = result
                            .servers
                            .into_iter()
                            .map(|entry| McpCatalogItem {
                                id: entry.id.clone(),
                                name: entry.name.clone(),
                                description: entry.description.clone().unwrap_or_default(),
                                source: entry.source.clone(),
                                entry,
                            })
                            .collect();
                    }
                    Err(error) => {
                        view.mcp_catalog.clear();
                        catalog_errors.push(format!("mcp browse: {error}"));
                    }
                }

                match skills {
                    Ok(items) if items.is_empty() => {
                        view.skills_installed_preview =
                            wormhole_i18n::t("settings.connections.skills.empty");
                    }
                    Ok(items) => {
                        view.skills_installed_preview = format!(
                            "{} ({})\n{}",
                            wormhole_i18n::t("settings.connections.skills.installed"),
                            items.len(),
                            items
                                .iter()
                                .map(|s| format!("• {} [{}]", s.name, s.id))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                    }
                    Err(error) => {
                        view.skills_installed_preview = format!("skills: {error}");
                    }
                }

                match skills_browse {
                    Ok(result) => {
                        view.skills_catalog = result
                            .skills
                            .into_iter()
                            .map(|entry| SkillCatalogItem {
                                id: entry.id.clone(),
                                name: entry.name.clone(),
                                description: entry.description.clone().unwrap_or_default(),
                                entry,
                            })
                            .collect();
                    }
                    Err(error) => {
                        view.skills_catalog.clear();
                        catalog_errors.push(format!("skills browse: {error}"));
                    }
                }

                if let Err(error) = channels {
                    catalog_errors.push(format!("channels: {error}"));
                }

                if catalog_errors.is_empty() {
                    if view.connections_message.is_empty() {
                        view.connections_message =
                            wormhole_i18n::t("settings.connections.ready");
                        view.connections_tone = StatusTone::Placeholder;
                    }
                } else {
                    view.connections_message = catalog_errors.join("; ");
                    view.connections_tone = StatusTone::Danger;
                }
                ctx.notify();
            },
        );
    }

    fn refresh_cluster(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cluster_status_fast(&state).await
            },
            |view, output, ctx| {
                match output {
                    Ok(status) => {
                        let active = status.clusters.into_iter().find(|cluster| cluster.active);
                        view.cluster_id = active
                            .as_ref()
                            .map(|cluster| cluster.cluster_id.clone())
                            .or(status.cluster_id);
                        view.cluster_name =
                            active.and_then(|cluster| cluster.name.or(Some(cluster.folder_name)));
                        view.cluster_message.clear();
                    }
                    Err(error) => {
                        view.cluster_message = format!("读取集群状态失败: {error}");
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_cache(&mut self, ctx: &mut ViewContext<Self>) {
        self.cache_busy = true;
        if self.cache_message.is_empty() {
            self.cache_message = "正在统计可安全重建的本地缓存…".into();
            self.cache_tone = StatusTone::Placeholder;
        }
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                settings_cache_status(&state).await
            },
            |view, output, ctx| {
                view.cache_busy = false;
                match output {
                    Ok(status) => {
                        view.cache_status = Some(status);
                        view.cache_message =
                            "仅统计聊天派生缓存、预览缓存与工作区传输缓存。".into();
                        view.cache_tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.cache_message = format!("缓存统计失败: {error}");
                        view.cache_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn sync_update_ui_from_shared(&mut self) {
        let shared = self
            .core
            .update()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        self.update_busy = shared.busy
            || matches!(
                shared.phase,
                DesktopUpdatePhase::Checking
                    | DesktopUpdatePhase::Downloading
                    | DesktopUpdatePhase::Installing
            );
        self.update_status = shared.status.clone();
        match shared.phase {
            DesktopUpdatePhase::Idle => {}
            DesktopUpdatePhase::Checking => {
                self.update_message = wormhole_i18n::t("settings.about.checking_status");
                self.update_tone = StatusTone::Placeholder;
            }
            DesktopUpdatePhase::Available => {
                if let Some(status) = &shared.status {
                    let latest = status
                        .latest_version
                        .clone()
                        .unwrap_or_else(|| wormhole_i18n::t("settings.about.unknown_version"));
                    self.update_message = wormhole_i18n::t_args(
                        "settings.about.update_available",
                        &[
                            ("latest", latest.as_str()),
                            ("current", status.current_version.as_str()),
                        ],
                    );
                    self.update_tone = StatusTone::Success;
                }
            }
            DesktopUpdatePhase::Downloading => {
                self.update_message = wormhole_i18n::t("settings.about.downloading");
                self.update_tone = StatusTone::Placeholder;
            }
            DesktopUpdatePhase::ReadyToInstall => {
                let version = shared
                    .ready_version()
                    .unwrap_or_else(|| wormhole_i18n::t("settings.about.unknown_version"));
                self.update_message = wormhole_i18n::t_args(
                    "settings.about.ready_to_install",
                    &[("version", version.as_str())],
                );
                self.update_tone = StatusTone::Success;
            }
            DesktopUpdatePhase::Installing => {
                self.update_message = wormhole_i18n::t("settings.about.installing");
                self.update_tone = StatusTone::Placeholder;
            }
            DesktopUpdatePhase::Error => {
                let error = shared
                    .error
                    .unwrap_or_else(|| wormhole_i18n::t("settings.about.unknown_version"));
                self.update_message = wormhole_i18n::t_args(
                    "settings.about.update_failed",
                    &[("error", error.as_str())],
                );
                self.update_tone = StatusTone::Danger;
            }
        }
        if shared.phase == DesktopUpdatePhase::Idle {
            if let Some(status) = &shared.status {
                if status.manifest_url.is_none() {
                    self.update_message = wormhole_i18n::t("settings.about.no_manifest");
                    self.update_tone = StatusTone::Placeholder;
                } else if !status.update_available {
                    self.update_message = wormhole_i18n::t_args(
                        "settings.about.up_to_date",
                        &[("version", status.current_version.as_str())],
                    );
                    self.update_tone = StatusTone::Success;
                }
            }
        }
    }

    fn begin_update_check(&mut self, ctx: &mut ViewContext<Self>, auto_download: bool) {
        {
            let mut shared = self
                .core
                .update()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if shared.busy
                || matches!(
                    shared.phase,
                    DesktopUpdatePhase::Downloading | DesktopUpdatePhase::Installing
                )
            {
                return;
            }
            shared.busy = true;
            shared.phase = DesktopUpdatePhase::Checking;
            shared.error = None;
        }
        self.sync_update_ui_from_shared();
        ctx.notify();
        ctx.emit(SettingsEvent::UpdateSessionChanged);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                check_desktop_update(&runtime).await
            },
            move |view, output, ctx| {
                let should_download = match output {
                    Ok(status) => {
                        let mut shared = view
                            .core
                            .update()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        shared.busy = false;
                        shared.status = Some(status.clone());
                        if status.manifest_url.is_none() {
                            shared.phase = DesktopUpdatePhase::Idle;
                            false
                        } else if status.update_available {
                            shared.phase = DesktopUpdatePhase::Available;
                            auto_download
                        } else {
                            shared.phase = DesktopUpdatePhase::Idle;
                            false
                        }
                    }
                    Err(error) => {
                        let mut shared = view
                            .core
                            .update()
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        shared.busy = false;
                        shared.phase = DesktopUpdatePhase::Error;
                        shared.error = Some(error);
                        false
                    }
                };
                view.sync_update_ui_from_shared();
                ctx.notify();
                ctx.emit(SettingsEvent::UpdateSessionChanged);
                if should_download {
                    view.begin_update_download(ctx);
                }
            },
        );
    }

    fn check_desktop_update(&mut self, ctx: &mut ViewContext<Self>) {
        self.begin_update_check(ctx, true);
    }

    fn begin_update_download(&mut self, ctx: &mut ViewContext<Self>) {
        {
            let mut shared = self
                .core
                .update()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if shared.busy
                || matches!(
                    shared.phase,
                    DesktopUpdatePhase::Downloading | DesktopUpdatePhase::Installing
                )
            {
                return;
            }
            shared.busy = true;
            shared.phase = DesktopUpdatePhase::Downloading;
            shared.error = None;
            shared.banner_dismissed = false;
        }
        self.sync_update_ui_from_shared();
        ctx.notify();
        ctx.emit(SettingsEvent::UpdateSessionChanged);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                download_desktop_update(&runtime).await
            },
            |view, output, ctx| {
                {
                    let mut shared = view
                        .core
                        .update()
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    shared.busy = false;
                    match output {
                        Ok(download) => {
                            shared.download = Some(download.clone());
                            if download.ready {
                                shared.phase = DesktopUpdatePhase::ReadyToInstall;
                                shared.error = None;
                            } else {
                                shared.phase = DesktopUpdatePhase::Idle;
                            }
                        }
                        Err(error) => {
                            shared.phase = DesktopUpdatePhase::Error;
                            shared.error = Some(error);
                        }
                    }
                }
                view.sync_update_ui_from_shared();
                ctx.notify();
                ctx.emit(SettingsEvent::UpdateSessionChanged);
            },
        );
    }

    fn begin_update_install(&mut self, ctx: &mut ViewContext<Self>) {
        {
            let mut shared = self
                .core
                .update()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if !matches!(shared.phase, DesktopUpdatePhase::ReadyToInstall) {
                return;
            }
            shared.busy = true;
            shared.phase = DesktopUpdatePhase::Installing;
            shared.error = None;
        }
        self.sync_update_ui_from_shared();
        ctx.notify();
        ctx.emit(SettingsEvent::UpdateSessionChanged);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let runtime = core.runtime();
                install_desktop_update(&runtime).await
            },
            |view, output, ctx| {
                match output {
                    Ok(()) => {
                        std::process::exit(0);
                    }
                    Err(error) => {
                        {
                            let mut shared = view
                                .core
                                .update()
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            shared.busy = false;
                            shared.phase = DesktopUpdatePhase::Error;
                            shared.error = Some(error);
                        }
                        view.sync_update_ui_from_shared();
                        ctx.notify();
                        ctx.emit(SettingsEvent::UpdateSessionChanged);
                    }
                }
            },
        );
    }

    fn refresh_relay(&mut self, ctx: &mut ViewContext<Self>) {
        self.relay_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                get_network_relay_status(&state).await
            },
            |view, output, ctx| {
                view.relay_busy = false;
                match output {
                    Ok(status) => {
                        view.relay_mode = status.relay_mode.clone();
                        view.relay_status = Some(status);
                        if view.relay_message.is_empty() {
                            view.relay_message =
                                "切换 Relay 后会重启 P2P 节点；集群成员需使用相同 Relay。".into();
                            view.relay_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(err) => {
                        view.relay_message = format!("读取 Relay 配置失败: {err}");
                        view.relay_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                shared_storage_info(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(s) => {
                        let was_dirty = view.storage_dirty();
                        if !was_dirty {
                            view.storage_draft = s.sync_entry_path.clone();
                        }
                        view.storage = Some(s);
                        if view.status.is_empty() {
                            view.status = "点击「浏览…」可选择新位置并迁移共享文件。".into();
                            view.status_tone = StatusTone::Placeholder;
                        }
                    }
                    Err(e) => {
                        view.status = format!("无法读取同步路径: {e}");
                        view.status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn path_row(&self, label: &str, value: &str) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::mono(value.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        );
        col.finish()
    }

    fn storage_dirty(&self) -> bool {
        self.storage
            .as_ref()
            .map(|storage| storage_paths_differ(&storage.sync_entry_path, &self.storage_draft))
            .unwrap_or(false)
    }

    fn flat_section(&self, body: Box<dyn Element>) -> Box<dyn Element> {
        Container::new(body)
            .with_padding_top(4.0)
            .with_padding_bottom(8.0)
            .finish()
    }

    fn page_header(&self, page: SettingsPage) -> Box<dyn Element> {
        let (eyebrow, sub) = page.eyebrow();
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            warpui::elements::Text::new(page.title().to_string(), self.font, SETTINGS_PANE_TITLE_SIZE)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::mono(format!("{eyebrow} {sub}"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(6.0)
            .with_margin_bottom(20.0)
            .finish(),
        );
        col.finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let draft = self.search_query.clone();
        let marked = self.search_field.marked_text.clone();
        let placeholder = wormhole_i18n::t("settings.search.placeholder");
        let field = TextFieldInput::builder(
            EventHandler::new(
                Container::new(render_field_with_caret(
                    &draft,
                    &marked,
                    &placeholder,
                    self.font,
                    self.search_focused,
                    false,
                    true,
                    self.search_field.cursor,
                ))
                .with_uniform_padding(9.0)
                .with_horizontal_padding(12.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(if self.search_focused {
                    theme::accent_cool()
                } else {
                    theme::border()
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                .finish(),
            )
            .with_automation_label("搜索设置")
            .with_automation_id("settings:search")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(SettingsAction::FocusSearch);
                DispatchEventResult::StopPropagation
            })
            .finish(),
            |ctx, action| {
                ctx.dispatch_typed_action(SettingsAction::SearchFieldEdit(action));
            },
        )
        .focused(self.search_focused)
        .disabled(false)
        .ime_preedit(!marked.is_empty())
        .finish();
        field
    }

    fn nav_item(&self, page: SettingsPage) -> Box<dyn Element> {
        let selected = self.selected_page == page;
        let mouse_state: MouseStateHandle = Arc::new(Mutex::new(MouseState::default()));
        let label = page.title().to_string();
        let nav_label = label.clone();
        let automation_id = page.nav_automation_id().to_string();
        let icon_path = page.icon_path();
        let font = self.font;
        Hoverable::new(mouse_state, move |state| {
            let hovered = state.is_hovered();
            let fg = if selected || hovered {
                theme::text()
            } else {
                theme::muted()
            };
            let border = if selected {
                theme::text()
            } else {
                ColorU::transparent_black()
            };
            let bg = if selected {
                theme::panel_elevated()
            } else if hovered {
                theme::accent_cool_bg(10)
            } else {
                ColorU::transparent_black()
            };
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max);
            row.add_child(icons::icon(icon_path, 16.0, fg));
            row.add_child(
                Container::new(
                    ui_text::body(label.clone(), font)
                        .with_color(fg)
                        .finish(),
                )
                .with_margin_left(10.0)
                .finish(),
            );
            Container::new(row.finish())
                .with_uniform_padding(8.0)
                .with_horizontal_padding(10.0)
                .with_background(bg)
                .with_border(Border::all(1.0).with_border_fill(border))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                .finish()
        })
        .with_automation_label(nav_label)
        .with_automation_id(automation_id)
        .on_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(SettingsAction::SelectPage(page));
        })
        .finish()
    }

    fn sidebar(&self) -> Box<dyn Element> {
        let mut nav_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        let mut visible_total = 0usize;
        for group_key in settings_nav_group_keys() {
            let pages: Vec<SettingsPage> = settings_pages_in_group_key(group_key)
                .into_iter()
                .filter(|page| page.matches_query(&self.search_query))
                .collect();
            if pages.is_empty() {
                continue;
            }
            visible_total += pages.len();
            let group_label = wormhole_i18n::t(group_key);
            nav_col.add_child(
                Container::new(
                    ui_text::mono(group_label, self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_padding_top(12.0)
                .with_padding_bottom(4.0)
                .with_horizontal_padding(10.0)
                .finish(),
            );
            for page in pages {
                nav_col.add_child(
                    Container::new(self.nav_item(page))
                        .with_margin_bottom(2.0)
                        .finish(),
                );
            }
        }
        if visible_total == 0 {
            nav_col.add_child(
                Container::new(
                    ui_text::mono(wormhole_i18n::t("settings.search.empty"), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_uniform_padding(12.0)
                .finish(),
            );
        }

        let nav_list = ClippedScrollable::vertical(
            self.nav_scroll.clone(),
            Container::new(nav_col.finish())
                .with_horizontal_padding(12.0)
                .with_padding_bottom(16.0)
                .finish(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();

        let mut shell = Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        shell.add_child(
            Container::new(self.search_box())
                .with_horizontal_padding(12.0)
                .with_padding_top(16.0)
                .with_padding_bottom(4.0)
                .finish(),
        );
        shell.add_child(Expanded::new(1.0, nav_list).finish());

        ConstrainedBox::new(
            Container::new(shell.finish())
                .with_background(theme::bg())
                .with_border(Border::right(1.0).with_border_fill(theme::border()))
                .finish(),
        )
        .with_width(SETTINGS_SIDEBAR_WIDTH)
        .finish()
    }

    fn page_body(&self, system_is_dark: bool) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.page_header(self.selected_page));
        match self.selected_page {
            SettingsPage::Account => col.add_child(self.account_block()),
            SettingsPage::Security => col.add_child(ChildView::new(&self.security).finish()),
            SettingsPage::Email => col.add_child(self.email_connectors_block()),
            SettingsPage::Connections => col.add_child(self.connections_block()),
            SettingsPage::Agent => col.add_child(ChildView::new(&self.agent_providers).finish()),
            SettingsPage::Memory => col.add_child(ChildView::new(&self.memory).finish()),
            SettingsPage::Activity => col.add_child(ChildView::new(&self.activity).finish()),
            SettingsPage::Subconscious => {
                col.add_child(ChildView::new(&self.subconscious).finish())
            }
            SettingsPage::TokenJuice => col.add_child(ChildView::new(&self.tokenjuice).finish()),
            SettingsPage::WebSearch => col.add_child(ChildView::new(&self.web_search).finish()),
            SettingsPage::Cron => col.add_child(ChildView::new(&self.cron).finish()),
            SettingsPage::Cluster => col.add_child(self.cluster_block()),
            SettingsPage::Relay => col.add_child(self.relay_block()),
            SettingsPage::SharedPath => col.add_child(self.shared_path_block()),
            SettingsPage::Cache => col.add_child(self.cache_block()),
            SettingsPage::Archive => col.add_child(self.archive_block()),
            SettingsPage::VirtualMachine => col.add_child(ChildView::new(&self.toolbox).finish()),
            SettingsPage::RdpHost => col.add_child(self.rdp_host_block()),
            SettingsPage::Display => col.add_child(ChildView::new(&self.display).finish()),
            SettingsPage::Plugins => col.add_child(ChildView::new(&self.plugins).finish()),
            SettingsPage::Theme => {
                let prefs = crate::ui::desktop_prefs::load(&self.core.data_dir());
                col.add_child(crate::ui::theme_studio::render_panel(
                    &prefs,
                    &self.theme_studio,
                    self.font,
                    system_is_dark,
                ));
            }
            SettingsPage::Language => col.add_child(self.language_block()),
            SettingsPage::About => col.add_child(self.about_block()),
        }
        col.add_child(
            Container::new(
                ui_text::body(crate::ui::fonts::UI_FONT_ATTRIBUTION, self.font)
                    .with_color(theme::placeholder())
                    .finish(),
            )
            .with_margin_top(24.0)
            .finish(),
        );
        col.finish()
    }

    /// Inline action buttons (`.settings-auth-actions` / `.settings-action-row` in HTML).
    fn inline_action_row(&self, buttons: Vec<Box<dyn Element>>) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        for (index, button) in buttons.into_iter().enumerate() {
            if index > 0 {
                row.add_child(Container::new(button).with_margin_left(8.0).finish());
            } else {
                row.add_child(button);
            }
        }
        row.finish()
    }

    fn action_button(&self, label: &str, action: SettingsAction) -> Box<dyn Element> {
        self.stateful_action_button(label, action, false, false)
    }

    fn copyable_id_row(
        &self,
        label: &str,
        automation_id: &str,
        action: SettingsAction,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        let automation_id = automation_id.to_string();
        Container::new(
            EventHandler::new(
                ui_text::mono(label.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_top(4.0)
        .with_padding_bottom(4.0)
        .finish()
    }

    fn stateful_action_button(
        &self,
        label: &str,
        action: SettingsAction,
        disabled: bool,
        primary: bool,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        let automation_id = format!("settings:btn:{label}");
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(7.0)
        .with_padding_bottom(7.0)
        .with_background(if primary {
            theme::accent_cool_bg(if disabled { 16 } else { 40 })
        } else {
            theme::accent_bg(if disabled { 8 } else { 24 })
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn relay_option_button(&self, mode: &str, label: &str) -> Box<dyn Element> {
        let selected = self.relay_mode == mode;
        let disabled = self.relay_busy;
        let mode = mode.to_string();
        let display_label = if selected {
            format!("● {label}")
        } else {
            format!("○ {label}")
        };
        let automation_id = format!("settings:relay:{mode}");
        Container::new(
            EventHandler::new(
                ui_text::body(display_label, self.font)
                    .with_color(if selected {
                        theme::accent_cool()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_automation_label(label.to_string())
            .with_automation_id(automation_id)
            .on_left_mouse_down({
                let mode = mode.clone();
                move |ctx, _, _| {
                    if disabled {
                        return DispatchEventResult::StopPropagation;
                    }
                    ctx.dispatch_typed_action(SettingsAction::SelectRelay(mode.clone()));
                    DispatchEventResult::StopPropagation
                }
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(if selected {
            theme::accent_cool_bg(24)
        } else {
            theme::accent_bg(12)
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn fold_summary_badge(&self, summary: &str) -> Box<dyn Element> {
        Container::new(
            ui_text::mono(summary.to_string(), self.font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_left(8.0)
        .with_padding_right(8.0)
        .with_padding_top(2.0)
        .with_padding_bottom(2.0)
        .with_background(theme::accent_cool_bg(36))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
        .finish()
    }

    fn collapsible_row(
        &self,
        label: &str,
        summary: &str,
        expanded: bool,
        action: SettingsAction,
    ) -> Box<dyn Element> {
        let chevron_path = settings_fold_chevron_path(expanded);
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(icons::icon(
            chevron_path,
            SETTINGS_FOLD_CHEVRON_SIZE,
            theme::muted(),
        ));
        row.add_child(
            Container::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
        row.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        row.add_child(self.fold_summary_badge(summary));
        let fold_label = label.to_string();
        let fold_id = format!("settings:fold:{fold_label}");
        let row = EventHandler::new(
            Container::new(row.finish())
                .with_padding_left(12.0)
                .with_padding_right(12.0)
                .with_padding_top(9.0)
                .with_padding_bottom(9.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                .finish(),
        )
        .with_automation_label(fold_label)
        .with_automation_id(fold_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish();
        ConstrainedBox::new(row)
            .with_max_width(SETTINGS_FOLD_TOGGLE_MAX_WIDTH)
            .finish()
    }

    fn relay_details(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "集群 P2P 穿透依赖 Relay。自动模式会并行探测国内/海外节点并选择最快可达者。",
            self.font,
        ));

        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        row.add_child(
            Container::new(self.relay_option_button("auto", "自动"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        row.add_child(
            Container::new(self.relay_option_button("domestic", "国内"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        row.add_child(
            Container::new(self.relay_option_button("overseas", "海外"))
                .with_horizontal_margin(4.0)
                .finish(),
        );
        col.add_child(row.finish());

        if let Some(status) = &self.relay_status {
            if status.env_override {
                col.add_child(
                    ui_text::body("环境变量 WORMHOLE_IROH_RELAYS 已接管 Relay", self.font)
                        .with_color(theme::danger())
                        .finish(),
                );
            }
            if let Some(url) = status.resolved_relay.as_deref() {
                col.add_child(self.path_row("当前生效", url));
            } else if !status.effective_urls.is_empty() {
                col.add_child(self.path_row("当前生效", &status.effective_urls.join(", ")));
            }
            for probe in &status.probe_results {
                let line = if probe.reachable {
                    format!(
                        "{} {} — {}ms",
                        probe.mode,
                        probe.url,
                        probe.latency_ms.unwrap_or(0)
                    )
                } else {
                    format!(
                        "{} {} — 不可达{}",
                        probe.mode,
                        probe.url,
                        probe
                            .error
                            .as_deref()
                            .map(|err| format!(" ({err})"))
                            .unwrap_or_default()
                    )
                };
                col.add_child(
                    ui_text::mono(line, self.font)
                        .with_color(if probe.reachable {
                            theme::muted()
                        } else {
                            theme::danger()
                        })
                        .finish(),
                );
            }
        }

        let mut actions = Flex::row();
        actions.add_child(
            Container::new(self.stateful_action_button(
                if self.relay_busy {
                    "正在应用…"
                } else {
                    "应用并重启 P2P"
                },
                SettingsAction::ApplyRelay,
                self.relay_busy,
                true,
            ))
            .with_horizontal_margin(4.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.stateful_action_button(
                "刷新",
                SettingsAction::RefreshRelay,
                self.relay_busy,
                false,
            ))
            .with_horizontal_margin(4.0)
            .finish(),
        );
        col.add_child(actions.finish());

        if !self.relay_message.is_empty() {
            col.add_child(status_line(
                self.relay_message.clone(),
                self.font,
                self.relay_tone,
            ));
        }
        ConstrainedBox::new(col.finish())
            .with_max_width(720.0)
            .finish()
    }

    fn relay_block(&self) -> Box<dyn Element> {
        let summary = relay_mode_label(&self.relay_mode);
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.collapsible_row(
            "网络穿透",
            summary,
            self.relay_expanded,
            SettingsAction::ToggleRelaySection,
        ));
        if self.relay_expanded {
            col.add_child(
                Container::new(self.relay_details())
                    .with_margin_top(8.0)
                    .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn shared_path_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "终端共享文件夹的本地副本写入此目录。点击「浏览…」选择新位置并迁移数据。",
            self.font,
        ));

        if let Some(info) = &self.storage {
            col.add_child(
                ui_text::body("当前同步目录", self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
            let draft = self.storage_draft.clone();
            let marked = self.storage_field.marked_text.clone();
            let field = TextFieldInput::builder(
                EventHandler::new(
                    Container::new(render_field_with_caret(
                        &draft,
                        &marked,
                        "选择共享文件存放位置",
                        self.font,
                        self.storage_focused,
                        self.busy,
                        true,
                        self.storage_field.cursor,
                    ))
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(if self.storage_dirty() {
                        theme::accent_cool()
                    } else {
                        theme::border()
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
                )
                .with_automation_label("共享目录路径")
                .with_automation_id("settings:storage_path")
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(SettingsAction::FocusPath);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
                |ctx, action| {
                    ctx.dispatch_typed_action(SettingsAction::TextFieldEdit(action));
                },
            )
            .focused(self.storage_focused)
            .disabled(self.busy)
            .ime_preedit(!marked.is_empty())
            .finish();
            let mut path_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max);
            path_row.add_child(Expanded::new(1.0, field).finish());
            path_row.add_child(
                Container::new(self.stateful_action_button(
                    "浏览…",
                    SettingsAction::BrowseMigrate,
                    self.busy,
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            path_row.add_child(
                Container::new(self.stateful_action_button(
                    if self.busy { "保存中…" } else { "保存" },
                    SettingsAction::SaveMigration,
                    self.busy || !self.storage_dirty(),
                    true,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            path_row.add_child(
                Container::new(self.action_button("刷新", SettingsAction::Refresh))
                    .with_margin_left(8.0)
                    .finish(),
            );
            col.add_child(
                ConstrainedBox::new(
                    Container::new(path_row.finish())
                        .with_vertical_margin(4.0)
                        .finish(),
                )
                .with_max_width(SETTINGS_FORM_MAX_WIDTH)
                .finish(),
            );
            if info.physical_path != info.sync_entry_path {
                col.add_child(
                    Container::new(self.path_row("物理存放", &info.physical_path))
                        .with_vertical_margin(6.0)
                        .finish(),
                );
            }
            if let Some(cluster) = &info.active_cluster_path {
                col.add_child(
                    Container::new(
                        ui_text::body(format!("当前集群目录: {cluster}"), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_vertical_margin(6.0)
                    .finish(),
                );
            }
        } else {
            col.add_child(
                ui_text::body("加载中…", self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        if !self.status.is_empty() {
            col.add_child(status_line(
                self.status.clone(),
                self.font,
                self.status_tone,
            ));
        }
        self.flat_section(col.finish())
    }

    fn account_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.auth.device_bind_hint"),
            self.font,
        ));
        if let Some(user_id) = &self.auth_user_id {
            col.add_child(self.copyable_id_row(
                &wormhole_i18n::t_args("settings.auth.user_id", &[("id", user_id)]),
                "settings:copy_user_id",
                SettingsAction::CopyUserId,
            ));
        }
        if let Some(device_id) = &self.auth_device_id {
            col.add_child(self.copyable_id_row(
                &wormhole_i18n::t_args("settings.auth.device_id", &[("id", device_id)]),
                "settings:copy_device_id",
                SettingsAction::CopyDeviceId,
            ));
        }
        if self.auth_user_id.is_some() || self.auth_device_id.is_some() {
            col.add_child(
                ui_text::device_meta(wormhole_i18n::t("settings.auth.copy_id_hint"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        }

        col.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("settings.auth.display_name_label"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(12.0)
            .finish(),
        );
        col.add_child(
            ui_text::device_meta(
                wormhole_i18n::t("settings.auth.display_name_hint"),
                self.font,
            )
            .with_color(theme::placeholder())
            .finish(),
        );
        let draft = self.account_display_name.clone();
        let marked = self.account_display_name_field.marked_text.clone();
        let placeholder = wormhole_i18n::t("settings.auth.display_name_placeholder");
        let name_field = TextFieldInput::builder(
            EventHandler::new(
                Container::new(render_field_with_caret(
                    &draft,
                    &marked,
                    &placeholder,
                    self.font,
                    self.account_display_name_focused,
                    self.account_display_name_busy || self.auth_user_id.is_none(),
                    true,
                    self.account_display_name_field.cursor,
                ))
                .with_uniform_padding(10.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(
                    if self.account_display_name_focused {
                        theme::accent_cool()
                    } else {
                        theme::border()
                    },
                ))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
            )
            .with_automation_label(wormhole_i18n::t("settings.auth.display_name_label"))
            .with_automation_id("settings:account_display_name")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(SettingsAction::FocusAccountDisplayName);
                DispatchEventResult::StopPropagation
            })
            .finish(),
            |ctx, action| {
                ctx.dispatch_typed_action(SettingsAction::AccountDisplayNameEdit(action));
            },
        )
        .focused(self.account_display_name_focused)
        .disabled(self.account_display_name_busy || self.auth_user_id.is_none())
        .ime_preedit(!marked.is_empty())
        .finish();
        let mut name_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        name_row.add_child(Expanded::new(1.0, name_field).finish());
        let save_label = if self.account_display_name_busy {
            wormhole_i18n::t("settings.action.saving")
        } else {
            wormhole_i18n::t("settings.action.save")
        };
        name_row.add_child(
            Container::new(self.stateful_action_button(
                &save_label,
                SettingsAction::SaveAccountDisplayName,
                self.account_display_name_busy || self.auth_user_id.is_none(),
                true,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(name_row.finish())
                .with_margin_top(8.0)
                .finish(),
        );

        if !self.auth_status.is_empty() {
            col.add_child(status_line(
                self.auth_status.clone(),
                self.font,
                self.auth_status_tone,
            ));
        }
        let mut auth_actions = Vec::new();
        if self.auth_user_id.is_none() {
            let login_label = if self.auth_busy {
                wormhole_i18n::t("settings.auth.logging_in")
            } else {
                wormhole_i18n::t("settings.auth.login")
            };
            auth_actions.push(self.stateful_action_button(
                &login_label,
                SettingsAction::Login,
                self.auth_busy,
                true,
            ));
        }
        let logout_label = if self.auth_busy && self.auth_user_id.is_some() {
            wormhole_i18n::t("settings.auth.signing_out")
        } else {
            wormhole_i18n::t("settings.auth.sign_out")
        };
        auth_actions.push(self.stateful_action_button(
            &logout_label,
            SettingsAction::Logout,
            self.auth_busy || self.auth_user_id.is_none(),
            false,
        ));
        col.add_child(
            Container::new(self.inline_action_row(auth_actions))
                .with_vertical_margin(8.0)
                .finish(),
        );
        let mut credit_actions = Vec::new();
        let purchase_label = wormhole_i18n::t("settings.action.purchase");
        credit_actions.push(self.stateful_action_button(
            &purchase_label,
            SettingsAction::OpenPurchase,
            self.auth_user_id.is_none(),
            true,
        ));
        let redeem_label = wormhole_i18n::t("settings.action.redeem");
        credit_actions.push(self.stateful_action_button(
            &redeem_label,
            SettingsAction::OpenRedeem,
            self.auth_user_id.is_none(),
            false,
        ));
        col.add_child(
            Container::new(self.inline_action_row(credit_actions))
                .with_vertical_margin(4.0)
                .finish(),
        );
        self.flat_section(col.finish())
    }

    fn rdp_host_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "配置无人值守、隐私屏、FPS、TOTP 与登录自启。完整 Host / Connect / 地址簿控制台在独立窗口中操作。",
            self.font,
        ));
        col.add_child(
            Container::new(self.stateful_action_button(
                "打开 Remote Desktop 控制台",
                SettingsAction::OpenRdpHostControl,
                false,
                true,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
        self.flat_section(col.finish())
    }

    fn language_block(&self) -> Box<dyn Element> {
        let data_dir = self.core.data_dir();
        let prefs = crate::ui::desktop_prefs::load(&data_dir);
        let current = prefs
            .ui_language
            .as_deref()
            .unwrap_or(wormhole_i18n::LOCALE_SYSTEM);
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.language.hint"),
            self.font,
        ));
        col.add_child(
            Container::new(
                ui_text::mono(wormhole_i18n::t("settings.language.apply_hint"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
        let choices = [
            (wormhole_i18n::LOCALE_SYSTEM, "settings.language.system"),
            (wormhole_i18n::LOCALE_ZH_CN, "settings.language.zh_cn"),
            (wormhole_i18n::LOCALE_EN, "settings.language.en"),
        ];
        for (value, label_key) in choices {
            let selected = current == value;
            let label = wormhole_i18n::t(label_key);
            let button_label = if selected {
                format!("✓ {label}")
            } else {
                label
            };
            let action_value = value.to_string();
            col.add_child(
                Container::new(self.stateful_action_button(
                    &button_label,
                    SettingsAction::SetUiLanguage(action_value),
                    false,
                    !selected,
                ))
                .with_margin_top(10.0)
                .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn about_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let current = self
            .update_status
            .as_ref()
            .map(|status| status.current_version.as_str())
            .unwrap_or(self.app_version.as_str());
        let version_label = wormhole_i18n::t_args(
            "settings.about.current_version",
            &[("version", current)],
        );
        col.add_child(
            EventHandler::new(
                ui_text::section_title(version_label.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_automation_id("settings:about_version")
            .with_automation_label(version_label)
            .on_left_mouse_down(|_, _, _| DispatchEventResult::PropagateToParent)
            .finish(),
        );
        col.add_child(
            Container::new(section_hint(
                wormhole_i18n::t("settings.about.update_hint"),
                self.font,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if let Some(status) = &self.update_status {
            if let Some(latest) = status.latest_version.as_deref() {
                col.add_child(
                    Container::new(
                        ui_text::mono(
                            wormhole_i18n::t_args(
                                "settings.about.latest_version",
                                &[("version", latest)],
                            ),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
            if let Some(url) = status.manifest_url.as_deref() {
                col.add_child(
                    Container::new(
                        ui_text::mono(
                            wormhole_i18n::t_args("settings.about.manifest_url", &[("url", url)]),
                            self.font,
                        )
                        .with_color(theme::placeholder())
                        .finish(),
                    )
                    .with_margin_top(6.0)
                    .finish(),
                );
            }
            if let Some(notes) = status.notes.as_deref().filter(|notes| !notes.is_empty()) {
                col.add_child(
                    Container::new(
                        ui_text::body(notes.to_string(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
        }
        if !self.update_message.is_empty() {
            col.add_child(status_line(
                self.update_message.clone(),
                self.font,
                self.update_tone,
            ));
        }
        let phase = self
            .core
            .update()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .phase;
        let check_label = if self.update_busy {
            match phase {
                DesktopUpdatePhase::Downloading => wormhole_i18n::t("settings.about.downloading"),
                DesktopUpdatePhase::Installing => wormhole_i18n::t("settings.about.installing"),
                _ => wormhole_i18n::t("settings.about.checking"),
            }
        } else {
            wormhole_i18n::t("settings.about.check_update")
        };
        let mut actions = vec![self.stateful_action_button(
            check_label.as_str(),
            SettingsAction::CheckDesktopUpdate,
            self.update_busy,
            true,
        )];
        if matches!(phase, DesktopUpdatePhase::ReadyToInstall) {
            let install_label = wormhole_i18n::t("settings.about.restart_and_install");
            actions.push(self.stateful_action_button(
                install_label.as_str(),
                SettingsAction::InstallDesktopUpdate,
                self.update_busy,
                false,
            ));
        }
        if matches!(phase, DesktopUpdatePhase::Error) {
            let retry_label = wormhole_i18n::t("settings.about.retry_download");
            actions.push(self.stateful_action_button(
                retry_label.as_str(),
                SettingsAction::RetryDesktopUpdateDownload,
                self.update_busy,
                false,
            ));
        }
        col.add_child(
            Container::new(self.inline_action_row(actions))
                .with_vertical_margin(8.0)
                .finish(),
        );
        self.flat_section(col.finish())
    }

    fn email_connector_row(&self, provider: &str, label: &str) -> Box<dyn Element> {
        let connection = self
            .email_connectors
            .iter()
            .find(|item| item.provider == provider && item.connected);
        let detail = connection
            .and_then(|item| item.email.as_deref())
            .unwrap_or("未连接")
            .to_string();
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        labels.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        labels.add_child(
            ui_text::mono(detail, self.font)
                .with_color(if connection.is_some() {
                    theme::accent_cool()
                } else {
                    theme::muted()
                })
                .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        if connection.is_some() {
            row.add_child(
                Container::new(self.stateful_action_button(
                    "测试读取",
                    SettingsAction::TestEmail(provider.to_string()),
                    self.email_busy,
                    false,
                ))
                .with_margin_right(8.0)
                .finish(),
            );
            row.add_child(self.stateful_action_button(
                "断开",
                SettingsAction::DisconnectEmail(provider.to_string()),
                self.email_busy,
                false,
            ));
        } else {
            row.add_child(self.stateful_action_button(
                "连接",
                SettingsAction::ConnectEmail(provider.to_string()),
                self.email_busy || self.auth_user_id.is_none(),
                true,
            ));
        }
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn email_connectors_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "通过官方 OAuth 连接 Gmail 或 Outlook。Wormhole 不保存邮箱密码；发送和修改邮件需要明确确认。",
            self.font,
        ));
        col.add_child(self.email_connector_row("gmail", "Gmail"));
        col.add_child(
            Container::new(self.email_connector_row("outlook", "Microsoft Outlook"))
                .with_margin_top(8.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.stateful_action_button(
                if self.email_busy {
                    "正在刷新…"
                } else {
                    "刷新连接状态"
                },
                SettingsAction::RefreshEmailConnectors,
                self.email_busy || self.auth_user_id.is_none(),
                false,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if !self.email_message.is_empty() {
            col.add_child(status_line(
                self.email_message.clone(),
                self.font,
                self.email_tone,
            ));
        }
        self.flat_section(col.finish())
    }

    fn connections_tab_button(&self, tab: u8, label: &str, automation_id: &str) -> Box<dyn Element> {
        let selected = self.connections_tab == tab;
        let disabled = self.connections_busy;
        let display_label = if selected {
            format!("● {label}")
        } else {
            format!("○ {label}")
        };
        let automation_id = automation_id.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(display_label, self.font)
                    .with_color(if selected {
                        theme::accent_cool()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_automation_label(label.to_string())
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
                ctx.dispatch_typed_action(SettingsAction::SelectConnectionsTab(tab));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(theme::accent_bg(if selected { 28 } else { 8 }))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn composio_catalog_row(&self, item: &ComposioCatalogItem) -> Box<dyn Element> {
        let connected = self
            .composio_connections
            .iter()
            .any(|conn| conn.toolkit.eq_ignore_ascii_case(&item.slug));
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let title = if item.native {
            format!(
                "{} · {}",
                item.name,
                wormhole_i18n::t("settings.connections.badge.native")
            )
        } else {
            format!(
                "{} · {}",
                item.name,
                wormhole_i18n::t("settings.connections.badge.proxied")
            )
        };
        labels.add_child(
            ui_text::body(title, self.font)
                .with_color(theme::text())
                .finish(),
        );
        let detail = if item.description.is_empty() {
            item.slug.clone()
        } else {
            format!("{} — {}", item.slug, item.description)
        };
        labels.add_child(
            ui_text::mono(detail, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        let connect_label = if connected {
            wormhole_i18n::t("settings.connections.manage")
        } else {
            wormhole_i18n::t("settings.connections.connect")
        };
        row.add_child(self.stateful_action_button(
            &connect_label,
            SettingsAction::ConnectComposio(item.slug.clone()),
            self.connections_busy || connected,
            !connected,
        ));
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn composio_connection_row(&self, item: &ComposioConnectionItem) -> Box<dyn Element> {
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        labels.add_child(
            ui_text::body(item.toolkit.clone(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        labels.add_child(
            ui_text::mono(
                format!("{} · {}", item.status, item.detail),
                self.font,
            )
            .with_color(theme::accent_cool())
            .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        let disconnect_label = wormhole_i18n::t("settings.connections.disconnect");
        row.add_child(self.stateful_action_button(
            &disconnect_label,
            SettingsAction::DisconnectComposio(item.id.clone()),
            self.connections_busy,
            false,
        ));
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn connections_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.connections.hint"),
            self.font,
        ));

        let tab_integrations = wormhole_i18n::t("settings.connections.tab.integrations");
        let tab_triggers = wormhole_i18n::t("settings.connections.tab.triggers");
        let tab_mcp = wormhole_i18n::t("settings.connections.tab.mcp");
        let tab_skills = wormhole_i18n::t("settings.connections.tab.skills");
        let tab_channels = wormhole_i18n::t("settings.connections.tab.channels");
        let tabs = self.inline_action_row(vec![
            self.connections_tab_button(
                0,
                &tab_integrations,
                "settings:connections_tab_integrations",
            ),
            self.connections_tab_button(
                1,
                &tab_triggers,
                "settings:connections_tab_triggers",
            ),
            self.connections_tab_button(2, &tab_mcp, "settings:connections_tab_mcp"),
            self.connections_tab_button(3, &tab_skills, "settings:connections_tab_skills"),
            self.connections_tab_button(4, &tab_channels, "settings:connections_tab_channels"),
        ]);
        col.add_child(
            Container::new(tabs)
                .with_margin_top(10.0)
                .with_margin_bottom(8.0)
                .finish(),
        );

        match self.connections_tab {
            0 => {
                if self.composio_catalog.is_empty() {
                    col.add_child(section_hint(
                        wormhole_i18n::t("settings.connections.catalog.empty"),
                        self.font,
                    ));
                } else {
                    for (index, item) in self.composio_catalog.iter().enumerate() {
                        let row = self.composio_catalog_row(item);
                        col.add_child(if index == 0 {
                            row
                        } else {
                            Container::new(row).with_margin_top(8.0).finish()
                        });
                    }
                }
                if !self.composio_connections.is_empty() {
                    col.add_child(
                        Container::new(
                            ui_text::body(
                                wormhole_i18n::t("settings.connections.active"),
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        )
                        .with_margin_top(14.0)
                        .with_margin_bottom(6.0)
                        .finish(),
                    );
                    for (index, item) in self.composio_connections.iter().enumerate() {
                        let row = self.composio_connection_row(item);
                        col.add_child(if index == 0 {
                            row
                        } else {
                            Container::new(row).with_margin_top(8.0).finish()
                        });
                    }
                }
                let refresh_label = if self.connections_busy {
                    wormhole_i18n::t("settings.connections.refreshing")
                } else {
                    wormhole_i18n::t("settings.connections.refresh")
                };
                col.add_child(
                    Container::new(self.stateful_action_button(
                        &refresh_label,
                        SettingsAction::RefreshConnections,
                        self.connections_busy,
                        false,
                    ))
                    .with_margin_top(10.0)
                    .finish(),
                );
            }
            1 => {
                let notify_label = if self.trigger_notify_agent {
                    wormhole_i18n::t("settings.connections.triggers.notify_on")
                } else {
                    wormhole_i18n::t("settings.connections.triggers.notify_off")
                };
                let drain_label = wormhole_i18n::t("settings.connections.triggers.drain");
                let sync_label = wormhole_i18n::t("settings.connections.triggers.sync");
                col.add_child(self.inline_action_row(vec![
                    self.stateful_action_button(
                        &notify_label,
                        SettingsAction::ToggleTriggerNotifyAgent,
                        self.connections_busy,
                        false,
                    ),
                    self.stateful_action_button(
                        &drain_label,
                        SettingsAction::DrainTriggers,
                        self.connections_busy,
                        true,
                    ),
                    self.stateful_action_button(
                        &sync_label,
                        SettingsAction::SyncIntegrations,
                        self.connections_busy,
                        false,
                    ),
                ]));
                if self.active_triggers.is_empty() {
                    col.add_child(
                        Container::new(section_hint(
                            wormhole_i18n::t("settings.connections.triggers.active_empty"),
                            self.font,
                        ))
                        .with_margin_top(10.0)
                        .finish(),
                    );
                } else {
                    for (index, item) in self.active_triggers.iter().enumerate() {
                        let mut labels = Flex::column()
                            .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                        labels.add_child(
                            ui_text::body(
                                format!("{} · {}", item.toolkit, item.slug),
                                self.font,
                            )
                            .with_color(theme::text())
                            .finish(),
                        );
                        labels.add_child(
                            ui_text::mono(
                                format!("{} · {}", item.state, item.id),
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        );
                        let mut row = Flex::row()
                            .with_cross_axis_alignment(CrossAxisAlignment::Center)
                            .with_main_axis_size(MainAxisSize::Max);
                        row.add_child(Expanded::new(1.0, labels.finish()).finish());
                        let disable_label =
                            wormhole_i18n::t("settings.connections.triggers.disable");
                        row.add_child(self.stateful_action_button(
                            &disable_label,
                            SettingsAction::DisableTrigger(item.id.clone()),
                            self.connections_busy,
                            false,
                        ));
                        let card = Container::new(row.finish())
                            .with_uniform_padding(10.0)
                            .with_border(Border::all(1.0).with_border_fill(theme::border()))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                            .finish();
                        col.add_child(
                            Container::new(card)
                                .with_margin_top(if index == 0 { 10.0 } else { 8.0 })
                                .finish(),
                        );
                    }
                }
                if !self.composio_connections.is_empty() {
                    col.add_child(
                        Container::new(
                            ui_text::body(
                                wormhole_i18n::t("settings.connections.triggers.create_hint"),
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        )
                        .with_margin_top(12.0)
                        .finish(),
                    );
                    for item in self.composio_connections.iter().take(6) {
                        let label = format!(
                            "{} · {}",
                            wormhole_i18n::t("settings.connections.triggers.create"),
                            item.toolkit
                        );
                        col.add_child(
                            Container::new(self.stateful_action_button(
                                &label,
                                SettingsAction::CreateTriggerForConnection(item.id.clone()),
                                self.connections_busy,
                                true,
                            ))
                            .with_margin_top(6.0)
                            .finish(),
                        );
                    }
                }
                col.add_child(
                    Container::new(
                        ui_text::mono(self.trigger_history_preview.clone(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(10.0)
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(theme::border()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
                );
            }
            2 => {
                let browse_label = wormhole_i18n::t("settings.connections.mcp.browse");
                col.add_child(self.stateful_action_button(
                    &browse_label,
                    SettingsAction::BrowseMcpCatalog,
                    self.connections_busy,
                    true,
                ));
                col.add_child(
                    Container::new(
                        ui_text::mono(self.mcp_installed_preview.clone(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(10.0)
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(theme::border()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
                );
                for (index, item) in self.mcp_catalog.iter().enumerate() {
                    let mut labels =
                        Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                    labels.add_child(
                        ui_text::body(item.name.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    );
                    let detail = if item.description.is_empty() {
                        format!("{} · {}", item.source, item.id)
                    } else {
                        format!("{} · {} — {}", item.source, item.id, item.description)
                    };
                    labels.add_child(
                        ui_text::mono(detail, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    );
                    let mut row = Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_main_axis_size(MainAxisSize::Max);
                    row.add_child(Expanded::new(1.0, labels.finish()).finish());
                    let install_label = wormhole_i18n::t("settings.connections.mcp.install");
                    row.add_child(self.stateful_action_button(
                        &install_label,
                        SettingsAction::InstallMcp(index),
                        self.connections_busy,
                        true,
                    ));
                    let uninstall_label = wormhole_i18n::t("settings.connections.mcp.uninstall");
                    row.add_child(self.stateful_action_button(
                        &uninstall_label,
                        SettingsAction::UninstallMcp(item.id.clone()),
                        self.connections_busy,
                        false,
                    ));
                    let card = Container::new(row.finish())
                        .with_uniform_padding(10.0)
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                        .finish();
                    col.add_child(
                        Container::new(card)
                            .with_margin_top(if index == 0 { 10.0 } else { 8.0 })
                            .finish(),
                    );
                }
            }
            3 => {
                let browse_label = wormhole_i18n::t("settings.connections.skills.browse");
                col.add_child(self.stateful_action_button(
                    &browse_label,
                    SettingsAction::BrowseSkillsCatalog,
                    self.connections_busy,
                    true,
                ));
                col.add_child(
                    Container::new(
                        ui_text::mono(self.skills_installed_preview.clone(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(10.0)
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(theme::border()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                    .finish(),
                );
                for (index, item) in self.skills_catalog.iter().enumerate() {
                    let mut labels =
                        Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                    labels.add_child(
                        ui_text::body(item.name.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    );
                    let detail = if item.description.is_empty() {
                        item.id.clone()
                    } else {
                        format!("{} — {}", item.id, item.description)
                    };
                    labels.add_child(
                        ui_text::mono(detail, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    );
                    let mut row = Flex::row()
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_main_axis_size(MainAxisSize::Max);
                    row.add_child(Expanded::new(1.0, labels.finish()).finish());
                    let install_label = wormhole_i18n::t("settings.connections.skills.install");
                    row.add_child(self.stateful_action_button(
                        &install_label,
                        SettingsAction::InstallSkill(index),
                        self.connections_busy,
                        true,
                    ));
                    let uninstall_label = wormhole_i18n::t("settings.connections.skills.uninstall");
                    row.add_child(self.stateful_action_button(
                        &uninstall_label,
                        SettingsAction::UninstallSkill(item.id.clone()),
                        self.connections_busy,
                        false,
                    ));
                    let card = Container::new(row.finish())
                        .with_uniform_padding(10.0)
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                        .finish();
                    col.add_child(
                        Container::new(card)
                            .with_margin_top(if index == 0 { 10.0 } else { 8.0 })
                            .finish(),
                    );
                }
            }
            _ => {
                col.add_child(section_hint(
                    wormhole_i18n::t("settings.connections.channels.hint"),
                    self.font,
                ));
                let current = format!(
                    "{}: {}",
                    wormhole_i18n::t("settings.connections.channels.current"),
                    self.channel_default
                );
                col.add_child(
                    Container::new(
                        ui_text::body(current, self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                );
                let web = wormhole_i18n::t("settings.connections.channels.web");
                let telegram = wormhole_i18n::t("settings.connections.channels.telegram");
                let discord = wormhole_i18n::t("settings.connections.channels.discord");
                col.add_child(
                    Container::new(self.inline_action_row(vec![
                        self.stateful_action_button(
                            &web,
                            SettingsAction::SetDefaultChannel("web".into()),
                            self.connections_busy,
                            self.channel_default != "web",
                        ),
                        self.stateful_action_button(
                            &telegram,
                            SettingsAction::SetDefaultChannel("telegram".into()),
                            self.connections_busy,
                            self.channel_default != "telegram",
                        ),
                        self.stateful_action_button(
                            &discord,
                            SettingsAction::SetDefaultChannel("discord".into()),
                            self.connections_busy,
                            self.channel_default != "discord",
                        ),
                    ]))
                    .with_margin_top(10.0)
                    .finish(),
                );
                col.add_child(
                    Container::new(section_hint(
                        wormhole_i18n::t("settings.connections.channels.connect_hint"),
                        self.font,
                    ))
                    .with_margin_top(12.0)
                    .finish(),
                );
                col.add_child(
                    Container::new(self.inline_action_row(vec![
                        self.stateful_action_button(
                            &telegram,
                            SettingsAction::ConnectComposio("telegram".into()),
                            self.connections_busy,
                            true,
                        ),
                        self.stateful_action_button(
                            &discord,
                            SettingsAction::ConnectComposio("discord".into()),
                            self.connections_busy,
                            true,
                        ),
                    ]))
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
        }

        if !self.connections_message.is_empty() {
            col.add_child(status_line(
                self.connections_message.clone(),
                self.font,
                self.connections_tone,
            ));
        }
        self.flat_section(col.finish())
    }

    fn cluster_block(&self) -> Box<dyn Element> {
        let summary = self
            .cluster_name
            .as_deref()
            .or(self.cluster_id.as_deref())
            .unwrap_or("未加入集群");
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.collapsible_row(
            "活动集群",
            summary,
            self.cluster_expanded,
            SettingsAction::ToggleClusterSection,
        ));
        if self.cluster_expanded {
            let mut details = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            if let Some(cluster_id) = self.cluster_id.as_deref() {
                details.add_child(self.path_row("Cluster ID", cluster_id));
            }
            if let Some(name) = self.cluster_name.as_deref() {
                details.add_child(
                    Container::new(self.path_row("当前名称", name))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
            if self.cluster_id.is_none() {
                details.add_child(section_hint("当前没有活动集群。", self.font));
            }
            details.add_child(
                Container::new(
                    self.action_button("前往终端管理", SettingsAction::OpenClusterManagement),
                )
                .with_margin_top(8.0)
                .finish(),
            );
            if !self.cluster_message.is_empty() {
                details.add_child(status_line(
                    self.cluster_message.clone(),
                    self.font,
                    StatusTone::Danger,
                ));
            }
            col.add_child(
                Container::new(
                    ConstrainedBox::new(details.finish())
                        .with_max_width(720.0)
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn cache_row(
        &self,
        label: &str,
        description: &str,
        bytes: Option<u64>,
        kind: &str,
    ) -> Box<dyn Element> {
        let mut labels = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        labels.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        labels.add_child(
            ui_text::body(description.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(Expanded::new(1.0, labels.finish()).finish());
        row.add_child(
            Container::new(
                ui_text::mono(
                    bytes
                        .map(format_cache_size)
                        .unwrap_or_else(|| "统计中…".into()),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_horizontal_margin(10.0)
            .finish(),
        );
        row.add_child(self.stateful_action_button(
            if self.cache_busy {
                "处理中…"
            } else {
                "清理"
            },
            SettingsAction::ClearCache(kind.to_string()),
            self.cache_busy || bytes.unwrap_or(0) == 0,
            false,
        ));
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish()
    }

    fn cache_block(&self) -> Box<dyn Element> {
        let summary = cache_summary_label(self.cache_status.as_ref());
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.collapsible_row(
            "缓存详情",
            &summary,
            self.cache_expanded,
            SettingsAction::ToggleCacheSection,
        ));
        if self.cache_expanded {
            let chat_bytes = self.cache_status.as_ref().map(|status| status.chat_bytes);
            let sync_bytes = self.cache_status.as_ref().map(|status| status.sync_bytes);
            let total = cache_total_bytes(self.cache_status.as_ref());
            let mut details = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            details.add_child(section_hint(
                "只清理可从聊天文档或远端文件重新生成的数据，不会删除共享文件、聊天索引或集群配置。",
                self.font,
            ));
            details.add_child(self.cache_row(
                "聊天记录缓存",
                "本地消息查询库与媒体派生文件",
                chat_bytes,
                "chat",
            ));
            details.add_child(
                Container::new(self.cache_row(
                    "终端同步文件缓存",
                    "预览文件与工作区传输临时文件",
                    sync_bytes,
                    "sync",
                ))
                .with_margin_top(6.0)
                .finish(),
            );
            let mut actions = Flex::row();
            actions.add_child(self.stateful_action_button(
                if self.cache_busy {
                    "正在清理…"
                } else {
                    "清理全部缓存"
                },
                SettingsAction::ClearCache("all".into()),
                self.cache_busy || total.unwrap_or(0) == 0,
                true,
            ));
            actions.add_child(
                Container::new(self.stateful_action_button(
                    "重新统计",
                    SettingsAction::RefreshCache,
                    self.cache_busy,
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            details.add_child(
                Container::new(actions.finish())
                    .with_margin_top(8.0)
                    .finish(),
            );
            if !self.cache_message.is_empty() {
                details.add_child(status_line(
                    self.cache_message.clone(),
                    self.font,
                    self.cache_tone,
                ));
            }
            col.add_child(
                Container::new(
                    ConstrainedBox::new(details.finish())
                        .with_max_width(720.0)
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        self.flat_section(col.finish())
    }

    fn archive_block(&self) -> Box<dyn Element> {
        let data_dir = self.core.data_dir();
        let snapshots = load_archived_snapshots(&data_dir);
        let count = snapshots.len();
        let count_label = count.to_string();

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            "从智能体归档的对话会显示在这里，可恢复至项目或独立对话，或永久删除。",
            self.font,
        ));
        col.add_child(self.collapsible_row(
            "已归档会话",
            &count_label,
            self.archive_expanded,
            SettingsAction::ToggleArchiveSection,
        ));

        if self.archive_expanded {
            let mut list_col =
                Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            if snapshots.is_empty() {
                list_col.add_child(
                    Container::new(section_hint("暂无归档会话", self.font))
                        .with_padding_top(4.0)
                        .with_padding_bottom(4.0)
                        .finish(),
                );
            } else {
                for snapshot in snapshots {
                    list_col.add_child(self.archived_session_row(snapshot));
                }
            }
            col.add_child(
                ConstrainedBox::new(
                    Container::new(list_col.finish())
                        .with_margin_top(4.0)
                        .with_uniform_padding(4.0)
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                        .finish(),
                )
                .with_max_width(520.0)
                .finish(),
            );
        }

        self.flat_section(col.finish())
    }

    fn archived_session_row(&self, snapshot: ArchivedSessionSnapshot) -> Box<dyn Element> {
        let session_id = snapshot.id.clone();
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        let mut label_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        label_col.add_child(
            ui_text::body(snapshot.label, self.font)
                .with_color(theme::text())
                .finish(),
        );
        label_col.add_child(
            ui_text::mono(snapshot.time, self.font)
                .with_color(theme::muted())
                .finish(),
        );
        row.add_child(Expanded::new(1.0, label_col.finish()).finish());
        row.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("恢复", self.font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .with_automation_label("恢复")
                .with_automation_id(format!("settings:archive_restore:{session_id}"))
                .on_left_mouse_down({
                    let id = session_id.clone();
                    move |ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::RestoreArchivedSession(
                            id.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    }
                })
                .finish(),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        row.add_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("永久删除", self.font)
                        .with_color(theme::danger())
                        .finish(),
                )
                .with_automation_label("永久删除")
                .with_automation_id(format!("settings:archive_delete:{session_id}"))
                .on_left_mouse_down({
                    let id = session_id.clone();
                    move |ctx, _, _| {
                        ctx.dispatch_typed_action(SettingsAction::DeleteArchivedSession(
                            id.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    }
                })
                .finish(),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
        Container::new(row.finish())
            .with_uniform_padding(8.0)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish()
    }
}

impl Entity for SettingsView {
    type Event = SettingsEvent;
}

impl View for SettingsView {
    fn ui_name() -> &'static str {
        "SettingsView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let system_is_dark =
            matches!(app.system_theme(), warpui::platform::SystemTheme::Dark);
        let pane = Container::new(
            ClippedScrollable::vertical(
                self.scroll.clone(),
                Container::new(self.page_body(system_is_dark))
                    .with_padding_top(28.0)
                    .with_padding_bottom(40.0)
                    .with_horizontal_padding(32.0)
                    .finish(),
                ScrollbarWidth::Auto,
                Fill::None,
                Fill::None,
                Fill::None,
            )
            .finish(),
        )
        .with_background(theme::panel())
        .finish();

        let mut shell = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        shell.add_child(self.sidebar());
        shell.add_child(Expanded::new(1.0, pane).finish());

        tab_content_fill(shell.finish())
    }
}

impl TypedActionView for SettingsView {
    type Action = SettingsAction;

    fn handle_action(&mut self, action: &SettingsAction, ctx: &mut ViewContext<Self>) {
        match action {
            SettingsAction::SelectPage(page) => {
                self.selected_page = *page;
                self.search_focused = false;
                self.storage_focused = false;
                self.account_display_name_focused = false;
                match page {
                    SettingsPage::Cluster => self.cluster_expanded = true,
                    SettingsPage::Relay => self.relay_expanded = true,
                    SettingsPage::Cache => self.cache_expanded = true,
                    SettingsPage::Archive => self.archive_expanded = true,
                    SettingsPage::Connections => self.refresh_connections(ctx),
                    SettingsPage::Account => self.refresh_account(ctx),
                    SettingsPage::About => self.check_desktop_update(ctx),
                    SettingsPage::Security => {
                        let security = self.security.clone();
                        ctx.update_view(&security, |view, ctx| {
                            view.reload(ctx);
                        });
                    }
                    _ => {}
                }
                ctx.notify();
            }
            SettingsAction::FocusSearch => {
                self.search_focused = true;
                self.storage_focused = false;
                self.account_display_name_focused = false;
                ctx.notify();
            }
            SettingsAction::FocusAccountDisplayName => {
                if !self.account_display_name_busy && self.auth_user_id.is_some() {
                    self.account_display_name_focused = true;
                    self.search_focused = false;
                    self.storage_focused = false;
                    ctx.notify();
                }
            }
            SettingsAction::AccountDisplayNameEdit(action) => {
                self.edit_account_display_name(action, ctx);
            }
            SettingsAction::SaveAccountDisplayName => {
                self.save_account_display_name(ctx);
            }
            SettingsAction::SearchFieldEdit(action) => {
                self.search_field.apply(&mut self.search_query, action);
                ctx.notify();
            }
            SettingsAction::Refresh => self.refresh(ctx),
            SettingsAction::RefreshAccount => self.refresh_account(ctx),
            SettingsAction::RefreshEmailConnectors => self.refresh_email_connectors(ctx),
            SettingsAction::RefreshConnections => self.refresh_connections(ctx),
            SettingsAction::SelectConnectionsTab(tab) => {
                self.connections_tab = *tab;
                ctx.notify();
            }
            SettingsAction::ConnectComposio(toolkit) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                self.connections_message = format!(
                    "{}: {toolkit}",
                    wormhole_i18n::t("settings.connections.authorizing")
                );
                self.connections_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                let toolkit = toolkit.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let result = composio_authorize(
                            &state,
                            ComposioAuthorizeParams {
                                toolkit,
                                extra_params: None,
                            },
                        )
                        .await?;
                        tokio::task::spawn_blocking(move || {
                            open_external_url(&result.connect_url)
                        })
                        .await
                        .map_err(|error| format!("启动浏览器失败: {error}"))??;
                        Ok::<(), String>(())
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(()) => {
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.authorize_opened");
                                view.connections_tone = StatusTone::Placeholder;
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.connect_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::DisconnectComposio(connection_id) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let core = self.core.clone();
                let connection_id = connection_id.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        composio_delete_connection(
                            &state,
                            ComposioConnectionIdParams { connection_id },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(_) => {
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.disconnected");
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.disconnect_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::ToggleTriggerNotifyAgent => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let next = !self.trigger_notify_agent;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        set_trigger_notify_prefs(
                            &state,
                            TriggerNotifyPrefs {
                                notify_agent: next,
                            },
                        )
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(prefs) => {
                                view.trigger_notify_agent = prefs.notify_agent;
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.triggers.prefs_saved");
                                view.connections_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.triggers.prefs_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::DrainTriggers => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                self.connections_message =
                    wormhole_i18n::t("settings.connections.triggers.draining");
                self.connections_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        composio_drain_triggers(&state).await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(value) => {
                                view.connections_message = format!(
                                    "{}: {value}",
                                    wormhole_i18n::t("settings.connections.triggers.drain_done")
                                );
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.triggers.drain_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::SyncIntegrations => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                self.connections_message = wormhole_i18n::t("settings.connections.syncing");
                self.connections_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        composio_sync(&state).await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(value) => {
                                view.connections_message = format!(
                                    "{}: {value}",
                                    wormhole_i18n::t("settings.connections.sync_done")
                                );
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.sync_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::DisableTrigger(trigger_id) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let core = self.core.clone();
                let trigger_id = trigger_id.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        composio_disable_trigger(
                            &state,
                            ComposioTriggerIdParams { trigger_id },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(_) => {
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.triggers.disabled");
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.triggers.disable_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::CreateTriggerForConnection(connection_id) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                self.connections_message =
                    wormhole_i18n::t("settings.connections.triggers.creating");
                self.connections_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                let connection_id = connection_id.clone();
                let toolkit = self
                    .composio_connections
                    .iter()
                    .find(|c| c.id == connection_id)
                    .map(|c| c.toolkit.clone())
                    .unwrap_or_default();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let available = wormhole_desktop_core::integrations_commands::composio_list_available_triggers(
                            &state,
                            Some(toolkit.clone()),
                        )
                        .await?;
                        let slug = available
                            .get("triggers")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|t| t.get("slug"))
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| "no available trigger for toolkit".to_string())?
                            .to_string();
                        let body = serde_json::json!({
                            "slug": slug,
                            "connectionId": connection_id,
                        });
                        composio_create_trigger(&state, body).await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(value) => {
                                view.connections_message = format!(
                                    "{}: {value}",
                                    wormhole_i18n::t("settings.connections.triggers.created")
                                );
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.triggers.create_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::BrowseMcpCatalog | SettingsAction::BrowseSkillsCatalog => {
                self.refresh_connections(ctx);
            }
            SettingsAction::InstallMcp(index) => {
                if self.connections_busy {
                    return;
                }
                let Some(item) = self.mcp_catalog.get(*index).cloned() else {
                    return;
                };
                self.connections_busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        mcp_install_entry(&state, item.entry)
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(server) => {
                                view.connections_message = format!(
                                    "{}: {}",
                                    wormhole_i18n::t("settings.connections.mcp.installed_ok"),
                                    server.name
                                );
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.mcp.install_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::UninstallMcp(id) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let core = self.core.clone();
                let id = id.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        mcp_uninstall(&state, McpIdParams { id })
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(_) => {
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.mcp.uninstalled");
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!("mcp uninstall: {error}");
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::InstallSkill(index) => {
                if self.connections_busy {
                    return;
                }
                let Some(item) = self.skills_catalog.get(*index).cloned() else {
                    return;
                };
                self.connections_busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        skills_install(&state, item.entry).await
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(skill) => {
                                view.connections_message = format!(
                                    "{}: {}",
                                    wormhole_i18n::t("settings.connections.skills.installed_ok"),
                                    skill.name
                                );
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.connections.skills.install_failed")
                                );
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::UninstallSkill(id) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let core = self.core.clone();
                let id = id.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        skills_uninstall(&state, McpIdParams { id })
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(_) => {
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.skills.uninstalled");
                                view.connections_tone = StatusTone::Success;
                                view.refresh_connections(ctx);
                            }
                            Err(error) => {
                                view.connections_message = format!("skills uninstall: {error}");
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::SetDefaultChannel(channel) => {
                if self.connections_busy {
                    return;
                }
                self.connections_busy = true;
                let core = self.core.clone();
                let channel = channel.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        set_channel_prefs(
                            &state,
                            SetChannelPrefsParams {
                                default_channel: channel,
                            },
                        )
                    },
                    |view, output, ctx| {
                        view.connections_busy = false;
                        match output {
                            Ok(prefs) => {
                                view.channel_default = prefs.default_channel.as_str().to_string();
                                view.connections_message =
                                    wormhole_i18n::t("settings.connections.channels.saved");
                                view.connections_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.connections_message = format!("channel: {error}");
                                view.connections_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::ConnectEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                self.email_message =
                    format!("正在创建 {} 授权链接…", email_provider_label(provider));
                self.email_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let result = email_connector_start_oauth(&state, &provider).await?;
                        tokio::task::spawn_blocking(move || {
                            open_external_url(&result.authorization_url)
                        })
                        .await
                        .map_err(|error| format!("启动浏览器失败: {error}"))??;
                        Ok::<(), String>(())
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(()) => {
                                view.email_message =
                                    "授权页已在系统浏览器打开；完成后点击“刷新连接状态”。".into();
                                view.email_tone = StatusTone::Placeholder;
                            }
                            Err(error) => {
                                view.email_message = format!("连接邮箱失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::DisconnectEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        email_connector_disconnect(&state, &provider).await
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(()) => {
                                view.email_message = "邮箱已断开。".into();
                                view.email_tone = StatusTone::Success;
                                view.refresh_email_connectors(ctx);
                            }
                            Err(error) => {
                                view.email_message = format!("断开邮箱失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::TestEmail(provider) => {
                if self.email_busy {
                    return;
                }
                self.email_busy = true;
                let core = self.core.clone();
                let provider = provider.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        email_connector_test_read(&state, &provider).await
                    },
                    |view, output, ctx| {
                        view.email_busy = false;
                        match output {
                            Ok(result) => {
                                view.email_message =
                                    format!("邮箱读取成功，返回 {} 封邮件。", result.count);
                                view.email_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.email_message = format!("邮箱读取失败: {error}");
                                view.email_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::RefreshRelay => self.refresh_relay(ctx),
            SettingsAction::RefreshCache => self.refresh_cache(ctx),
            SettingsAction::ToggleClusterSection => {
                self.cluster_expanded = !self.cluster_expanded;
                ctx.notify();
            }
            SettingsAction::OpenClusterManagement => {
                ctx.emit(SettingsEvent::OpenClusterManagement);
            }
            SettingsAction::OpenRdpHostControl => {
                ctx.emit(SettingsEvent::OpenRdpHostControl);
            }
            SettingsAction::CopyUserId => {
                match self.auth_user_id.as_deref() {
                    Some(user_id) if !user_id.is_empty() => match write_clipboard_text(user_id) {
                        Ok(()) => {
                            self.auth_status = wormhole_i18n::t("settings.auth.copied_user_id");
                            self.auth_status_tone = StatusTone::Success;
                        }
                        Err(err) => {
                            self.auth_status =
                                format!("{}: {err}", wormhole_i18n::t("settings.auth.copy_failed"));
                            self.auth_status_tone = StatusTone::Danger;
                        }
                    },
                    _ => {
                        self.auth_status = wormhole_i18n::t("settings.auth.copy_failed");
                        self.auth_status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            }
            SettingsAction::CopyDeviceId => {
                match self.auth_device_id.as_deref() {
                    Some(device_id) if !device_id.is_empty() => {
                        match write_clipboard_text(device_id) {
                            Ok(()) => {
                                self.auth_status =
                                    wormhole_i18n::t("settings.auth.copied_device_id");
                                self.auth_status_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                self.auth_status = format!(
                                    "{}: {err}",
                                    wormhole_i18n::t("settings.auth.copy_failed")
                                );
                                self.auth_status_tone = StatusTone::Danger;
                            }
                        }
                    }
                    _ => {
                        self.auth_status = wormhole_i18n::t("settings.auth.copy_failed");
                        self.auth_status_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            }
            SettingsAction::ToggleRelaySection => {
                self.relay_expanded = !self.relay_expanded;
                ctx.notify();
            }
            SettingsAction::ToggleCacheSection => {
                self.cache_expanded = !self.cache_expanded;
                ctx.notify();
            }
            SettingsAction::SelectRelay(mode) => {
                if is_relay_mode(mode) && !self.relay_busy {
                    self.relay_mode = mode.clone();
                    self.relay_message = "Relay 模式已修改，点击「应用并重启 P2P」生效。".into();
                    self.relay_tone = StatusTone::Placeholder;
                    ctx.notify();
                }
            }
            SettingsAction::ApplyRelay => {
                if self.relay_busy {
                    return;
                }
                self.relay_busy = true;
                self.relay_message = "正在应用 Relay 并重启 P2P 节点…".into();
                self.relay_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let relay_mode = self.relay_mode.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        save_network_relay_config(&state, SaveNetworkRelayParams { relay_mode })
                            .await
                    },
                    |view, output, ctx| {
                        view.relay_busy = false;
                        match output {
                            Ok(status) => {
                                view.relay_mode = status.relay_mode.clone();
                                view.relay_status = Some(status);
                                view.relay_message = "Relay 已应用，P2P 节点已重启。".into();
                                view.relay_tone = StatusTone::Success;
                            }
                            Err(err) => {
                                view.relay_message = format!("应用 Relay 失败: {err}");
                                view.relay_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::Login => {
                ctx.emit(SettingsEvent::OpenLogin);
            }
            SettingsAction::OpenPurchase => {
                ctx.emit(SettingsEvent::OpenPurchase);
            }
            SettingsAction::OpenRedeem => {
                ctx.emit(SettingsEvent::OpenRedeem);
            }
            SettingsAction::Logout => {
                self.auth_busy = true;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        clear_cloud_auth_token(&state).await
                    },
                    |view, output, ctx| {
                        view.auth_busy = false;
                        match output {
                            Ok(()) => {
                                view.auth_user_id = None;
                                view.auth_device_id = None;
                                view.account_display_name.clear();
                                view.account_display_name_field = TextFieldState::new();
                                view.account_display_name_focused = false;
                                view.auth_status = "已退出登录。".into();
                                view.auth_status_tone = StatusTone::Placeholder;
                                ctx.emit(SettingsEvent::AccountChanged {
                                    authenticated: false,
                                });
                            }
                            Err(err) => {
                                view.auth_status = format!("退出失败: {err}");
                                view.auth_status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::BrowseMigrate => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.status = "正在选择文件夹…".into();
                self.status_tone = StatusTone::Placeholder;
                ctx.notify();
                ctx.spawn(
                    async move {
                        let picked = tokio::task::spawn_blocking(|| {
                            #[cfg(windows)]
                            {
                                wormhole_desktop_platform_windows::pick_folder(
                                    "选择共享文件存放位置",
                                )
                            }
                            #[cfg(not(windows))]
                            {
                                rfd::FileDialog::new()
                                    .set_title("选择共享文件存放位置")
                                    .pick_folder()
                            }
                        })
                        .await
                        .ok()
                        .flatten();

                        let Some(path) = picked else {
                            return None;
                        };
                        Some(path.display().to_string())
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Some(path) => {
                                view.storage_draft = path;
                                view.storage_field.clear_marked();
                                view.storage_focused = true;
                                view.status = "已选择文件夹，点击「保存」开始迁移。".into();
                                view.status_tone = StatusTone::Placeholder;
                            }
                            None => {
                                view.status = "已取消选择。".into();
                                view.status_tone = StatusTone::Placeholder;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::SaveMigration => {
                if self.busy || !self.storage_dirty() {
                    return;
                }
                let target = self.storage_draft.trim().to_string();
                if target.is_empty() {
                    self.status = "共享文件路径不能为空。".into();
                    self.status_tone = StatusTone::Danger;
                    ctx.notify();
                    return;
                }
                self.busy = true;
                self.storage_focused = false;
                self.status = "正在迁移共享文件…".into();
                self.status_tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.notify();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        migrate_shared_storage(&state, target).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(info) => {
                                view.storage_draft = info.sync_entry_path.clone();
                                view.status = format!("已迁移共享文件至 {}", info.sync_entry_path);
                                view.storage = Some(info);
                                view.status_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.status = format!("迁移失败: {error}");
                                view.status_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::FocusPath => {
                if !self.busy {
                    self.storage_focused = true;
                    self.search_focused = false;
                    self.account_display_name_focused = false;
                    ctx.notify();
                }
            }
            SettingsAction::TextFieldEdit(action) => {
                if !self.busy {
                    self.storage_field.apply(&mut self.storage_draft, action);
                    self.status.clear();
                    ctx.notify();
                }
            }
            SettingsAction::ToggleArchiveSection => {
                self.archive_expanded = !self.archive_expanded;
                let value = if self.archive_expanded { "1" } else { "0" };
                if let Err(error) = std::fs::write(Self::archive_expanded_path(&self.core), value) {
                    self.status = format!("保存归档折叠状态失败: {error}");
                    self.status_tone = StatusTone::Danger;
                }
                ctx.notify();
            }
            SettingsAction::RestoreArchivedSession(id) => {
                ctx.emit(SettingsEvent::RestoreArchivedSession(id.clone()));
            }
            SettingsAction::DeleteArchivedSession(id) => {
                ctx.emit(SettingsEvent::DeleteArchivedSession(id.clone()));
            }
            SettingsAction::ClearCache(kind) => {
                if self.cache_busy {
                    return;
                }
                self.cache_busy = true;
                self.cache_message = "正在安全清理缓存…".into();
                self.cache_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                let kind = kind.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        clear_settings_cache(&state, ClearSettingsCacheParams { kind }).await
                    },
                    |view, output, ctx| {
                        view.cache_busy = false;
                        match output {
                            Ok(result) => {
                                view.cache_status = Some(result.status);
                                view.cache_message = format!(
                                    "缓存清理完成，已释放 {}。",
                                    format_cache_size(result.removed_bytes)
                                );
                                view.cache_tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.cache_message = format!("缓存清理失败: {error}");
                                view.cache_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            SettingsAction::CheckDesktopUpdate => self.check_desktop_update(ctx),
            SettingsAction::InstallDesktopUpdate => self.begin_update_install(ctx),
            SettingsAction::RetryDesktopUpdateDownload => self.begin_update_download(ctx),
            SettingsAction::SetUiLanguage(language) => {
                let data_dir = self.core.data_dir();
                if let Err(err) = crate::ui::desktop_prefs::set_ui_language(&data_dir, language) {
                    self.update_message = err;
                    self.update_tone = StatusTone::Danger;
                }
                ctx.notify();
            }
            SettingsAction::ThemeStudio(action) => {
                let data_dir = self.core.data_dir();
                crate::ui::theme_studio::handle_action_with_app(
                    &mut self.theme_studio,
                    &data_dir,
                    action.clone(),
                    &*ctx,
                );
                ctx.notify();
            }
        }
    }
}

fn storage_paths_differ(current: &str, draft: &str) -> bool {
    current.trim() != draft.trim()
}

fn email_provider_label(provider: &str) -> &'static str {
    match provider {
        "gmail" => "Gmail",
        "outlook" => "Microsoft Outlook",
        _ => "邮箱",
    }
}

fn open_external_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1:")) {
        return Err("拒绝打开非 HTTPS 授权地址".into());
    }
    #[cfg(windows)]
    let status = std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .status();
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    status
        .map_err(|error| error.to_string())?
        .success()
        .then_some(())
        .ok_or_else(|| "系统浏览器启动失败".into())
}

const SETTINGS_SIDEBAR_WIDTH: f32 = 240.0;
const SETTINGS_FOLD_TOGGLE_MAX_WIDTH: f32 = 420.0;
const SETTINGS_FORM_MAX_WIDTH: f32 = 560.0;
const SETTINGS_FOLD_CHEVRON_SIZE: f32 = 14.0;
/// Matches `.settings-pane-title` in `desktop-current.html`.
const SETTINGS_PANE_TITLE_SIZE: f32 = 28.0;

fn settings_fold_chevron_path(expanded: bool) -> &'static str {
    if expanded {
        "agent-chevron-down.svg"
    } else {
        "agent-chevron.svg"
    }
}

fn cache_total_bytes(status: Option<&SettingsCacheStatusDto>) -> Option<u64> {
    status.map(|entry| entry.chat_bytes.saturating_add(entry.sync_bytes))
}

fn cache_summary_label(status: Option<&SettingsCacheStatusDto>) -> String {
    cache_total_bytes(status)
        .map(format_cache_size)
        .unwrap_or_else(|| "统计中…".into())
}

fn is_relay_mode(mode: &str) -> bool {
    matches!(mode, "auto" | "domestic" | "overseas")
}

fn relay_mode_label(mode: &str) -> &'static str {
    match mode {
        "domestic" => "国内 Relay",
        "overseas" => "海外 Relay",
        _ => "自动选择",
    }
}

fn format_cache_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cache_summary_label, email_provider_label, format_cache_size, is_relay_mode,
        relay_mode_label, settings_fold_chevron_path, settings_pages_in_group_key,
        storage_paths_differ, SettingsPage,
    };
    use wormhole_desktop_core::settings_cache_commands::SettingsCacheStatusDto;

    #[test]
    fn settings_page_defaults_to_account_and_filters_search() {
        wormhole_i18n::set_locale("zh-CN");
        assert_eq!(SettingsPage::Account.title(), "账号");
        assert!(SettingsPage::Account.matches_query(""));
        assert!(SettingsPage::Account.matches_query("账号"));
        assert!(SettingsPage::Relay.matches_query("p2p"));
        assert!(SettingsPage::Email.matches_query("gmail"));
        assert!(SettingsPage::Plugins.matches_query("bb-browser"));
        assert!(SettingsPage::Agent.matches_query("codex"));
        assert!(!SettingsPage::Agent.matches_query("bb-browser"));
        assert!(SettingsPage::Agent.matches_query("openai"));
        assert!(SettingsPage::Plugins.matches_query("插件市场"));
        assert!(SettingsPage::VirtualMachine.matches_query("工具箱"));
        assert!(SettingsPage::VirtualMachine.matches_query("toolbox"));
        assert!(SettingsPage::VirtualMachine.matches_query("用户程序"));
        assert!(SettingsPage::VirtualMachine.matches_query("指定人"));
        assert!(!SettingsPage::Cache.matches_query("虚拟机"));
        assert!(SettingsPage::WebSearch.matches_query("brave"));
        assert!(SettingsPage::WebSearch.matches_query("exa"));
        assert!(SettingsPage::Activity.matches_query("alerts"));
        assert!(SettingsPage::Activity.matches_query("通知"));
        assert!(SettingsPage::Cron.matches_query("cron"));
        assert!(SettingsPage::Cron.matches_query("定时"));
        assert!(SettingsPage::TokenJuice.matches_query("ccr"));
        assert_eq!(
            settings_pages_in_group_key("settings.nav.group.general"),
            vec![
                SettingsPage::Account,
                SettingsPage::Security,
                SettingsPage::Email,
                SettingsPage::Connections,
                SettingsPage::Agent,
                SettingsPage::Theme,
                SettingsPage::Memory,
                SettingsPage::Activity,
                SettingsPage::Subconscious,
                SettingsPage::TokenJuice,
                SettingsPage::WebSearch,
                SettingsPage::Cron,
            ]
        );
        assert_eq!(
            settings_pages_in_group_key("settings.nav.group.cluster"),
            vec![SettingsPage::Cluster, SettingsPage::Relay]
        );
        assert_eq!(
            settings_pages_in_group_key("settings.nav.group.system"),
            vec![
                SettingsPage::VirtualMachine,
                SettingsPage::RdpHost,
                SettingsPage::Display,
                SettingsPage::Plugins,
                SettingsPage::Language,
                SettingsPage::About,
            ]
        );
        assert!(SettingsPage::RdpHost.matches_query("隐私屏"));
        assert!(SettingsPage::Display.matches_query("ipad"));
        assert!(SettingsPage::Theme.matches_query("主题"));
        assert!(SettingsPage::About.matches_query("检查更新"));
    }

    #[test]
    fn email_provider_labels_are_stable() {
        assert_eq!(email_provider_label("gmail"), "Gmail");
        assert_eq!(email_provider_label("outlook"), "Microsoft Outlook");
    }

    #[test]
    fn storage_dirty_state_ignores_outer_whitespace_only() {
        assert!(!storage_paths_differ("D:\\Wormhole", " D:\\Wormhole "));
        assert!(storage_paths_differ("D:\\Wormhole", "E:\\Wormhole"));
    }

    #[test]
    fn relay_mode_accepts_only_backend_modes() {
        assert!(is_relay_mode("auto"));
        assert!(is_relay_mode("domestic"));
        assert!(is_relay_mode("overseas"));
        assert!(!is_relay_mode("fastest"));
    }

    #[test]
    fn setting_summaries_are_compact_and_stable() {
        assert_eq!(relay_mode_label("auto"), "自动选择");
        assert_eq!(relay_mode_label("domestic"), "国内 Relay");
        assert_eq!(format_cache_size(0), "0 B");
        assert_eq!(format_cache_size(1536), "1.5 KB");
        assert_eq!(format_cache_size(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn settings_fold_chevron_paths_match_expanded_state() {
        assert_eq!(settings_fold_chevron_path(false), "agent-chevron.svg");
        assert_eq!(settings_fold_chevron_path(true), "agent-chevron-down.svg");
    }

    #[test]
    fn cache_summary_reports_total_or_pending() {
        assert_eq!(cache_summary_label(None), "统计中…");
        let status = SettingsCacheStatusDto {
            chat_bytes: 1024,
            sync_bytes: 2048,
        };
        assert_eq!(cache_summary_label(Some(&status)), "3.0 KB");
    }
}
