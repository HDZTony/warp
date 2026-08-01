use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    Empty, EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::toolbox_ui::{
    toolbox_cancel_install, toolbox_install_tool, toolbox_launch_tool, toolbox_list_tools,
    workspace_user_app_catalog_install, workspace_user_app_catalog_list,
    workspace_user_app_catalog_revoke, workspace_user_app_catalog_update_acl,
    workspace_user_app_import, workspace_user_app_list, workspace_user_app_publish,
    workspace_user_app_remove, parse_extension_list, CatalogUserAppSummary, ToolCategory,
    ToolExecutorKind, ToolInstallStage, ToolSourceKind, ToolSummary,
    WorkspaceUserAppCatalogInstallParams, WorkspaceUserAppCatalogRevokeParams,
    WorkspaceUserAppCatalogUpdateAclParams, WorkspaceUserAppIdParams,
    WorkspaceUserAppImportParams, WorkspaceUserAppManifest, WorkspaceUserAppPublishParams,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    section_hint, status_line, StatusTone, HUD_RADIUS, SECTION_GAP,
};
use crate::ui::text_field_input::{
    render_search_field_with_caret, sync_caret_blink, wrap_text_field_focus_on_click, CaretBlink,
    CaretBlinkHost, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum ToolboxAction {
    Refresh,
    Primary(String),
    Cancel(String),
    SetCategory(Option<ToolCategory>),
    SearchEdit(TextFieldEditAction),
    ActivateSearch,
    BlurSearch,
    RefreshUserApps,
    ImportPathEdit(TextFieldEditAction),
    ActivateImportPath,
    BlurImportPath,
    ImportDisplayNameEdit(TextFieldEditAction),
    ActivateImportDisplayName,
    BlurImportDisplayName,
    ImportExtensionsEdit(TextFieldEditAction),
    ActivateImportExtensions,
    BlurImportExtensions,
    ImportEntrypointEdit(TextFieldEditAction),
    ActivateImportEntrypoint,
    BlurImportEntrypoint,
    ImportVersionEdit(TextFieldEditAction),
    ActivateImportVersion,
    BlurImportVersion,
    AllowlistIdsEdit(TextFieldEditAction),
    ActivateAllowlistIds,
    BlurAllowlistIds,
    BrowseImportPath,
    ImportLocal,
    RemoveLocal(String),
    PublishLocal {
        app_id: String,
        visibility: String,
    },
    InstallCatalog {
        app_id: String,
        version: String,
    },
    UpdateCatalogAcl {
        app_id: String,
        version: String,
        visibility: String,
    },
    RevokeCatalog {
        app_id: String,
        version: String,
    },
}

pub struct ToolboxView {
    core: CoreHandle,
    font: FamilyId,
    mono: FamilyId,
    tools: Vec<ToolSummary>,
    loading: bool,
    busy_tool: Option<String>,
    message: String,
    message_tone: StatusTone,
    selected_category: Option<ToolCategory>,
    search_query: String,
    search_field: TextFieldState,
    search_focused: bool,
    caret_blink: CaretBlink,
    local_user_apps: Vec<WorkspaceUserAppManifest>,
    catalog_user_apps: Vec<CatalogUserAppSummary>,
    import_path: String,
    import_path_field: TextFieldState,
    import_path_focused: bool,
    import_display_name: String,
    import_display_name_field: TextFieldState,
    import_display_name_focused: bool,
    import_extensions: String,
    import_extensions_field: TextFieldState,
    import_extensions_focused: bool,
    import_entrypoint: String,
    import_entrypoint_field: TextFieldState,
    import_entrypoint_focused: bool,
    publish_version: String,
    publish_version_field: TextFieldState,
    publish_version_focused: bool,
    allowlist_ids: String,
    allowlist_ids_field: TextFieldState,
    allowlist_ids_focused: bool,
    user_apps_busy: bool,
}

impl ToolboxView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let mut view = Self {
            core,
            font,
            mono,
            tools: Vec::new(),
            loading: false,
            busy_tool: None,
            message: String::new(),
            message_tone: StatusTone::Placeholder,
            selected_category: None,
            search_query: String::new(),
            search_field: TextFieldState::new(),
            search_focused: false,
            caret_blink: CaretBlink::default(),
            local_user_apps: Vec::new(),
            catalog_user_apps: Vec::new(),
            import_path: String::new(),
            import_path_field: TextFieldState::new(),
            import_path_focused: false,
            import_display_name: String::new(),
            import_display_name_field: TextFieldState::new(),
            import_display_name_focused: false,
            import_extensions: String::new(),
            import_extensions_field: TextFieldState::new(),
            import_extensions_focused: false,
            import_entrypoint: String::new(),
            import_entrypoint_field: TextFieldState::new(),
            import_entrypoint_focused: false,
            publish_version: "1".into(),
            publish_version_field: TextFieldState::new(),
            publish_version_focused: false,
            allowlist_ids: String::new(),
            allowlist_ids_field: TextFieldState::new(),
            allowlist_ids_focused: false,
            user_apps_busy: false,
        };
        view.refresh(ctx);
        view.refresh_user_apps(ctx);
        view
    }

    fn refresh_user_apps(&mut self, ctx: &mut ViewContext<Self>) {
        self.user_apps_busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let local = workspace_user_app_list(&state).await;
                let catalog = workspace_user_app_catalog_list(&state).await;
                (local, catalog)
            },
            |view, output, ctx| {
                view.user_apps_busy = false;
                match output.0 {
                    Ok(apps) => view.local_user_apps = apps,
                    Err(error) => {
                        view.message = format!("读取本机用户程序失败: {error}");
                        view.message_tone = StatusTone::Danger;
                    }
                }
                match output.1 {
                    Ok(apps) => view.catalog_user_apps = apps,
                    Err(error) => {
                        // Catalog requires login; keep local list usable.
                        if view.message.is_empty() {
                            view.message = format!("用户程序目录暂不可用: {error}");
                            view.message_tone = StatusTone::Placeholder;
                        }
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.loading = true;
        if self.tools.is_empty() {
            self.message = "正在读取工具目录…".into();
            self.message_tone = StatusTone::Placeholder;
        }
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                toolbox_list_tools(&state).await
            },
            |view, output, ctx| {
                view.loading = false;
                match output {
                    Ok(tools) => {
                        view.tools = tools;
                        if view.message == "正在读取工具目录…" {
                            view.message.clear();
                        }
                    }
                    Err(error) => {
                        view.message = format!("读取工具目录失败: {error}");
                        view.message_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn filtered_tools(&self) -> Vec<&ToolSummary> {
        let query = self.search_query.trim().to_ascii_lowercase();
        self.tools
            .iter()
            .filter(|tool| {
                if let Some(category) = self.selected_category {
                    if !tool.descriptor.categories.contains(&category) {
                        return false;
                    }
                }
                if query.is_empty() {
                    return true;
                }
                let name = tool.descriptor.name.to_ascii_lowercase();
                let description = tool.descriptor.description.to_ascii_lowercase();
                let id = tool.descriptor.id.to_ascii_lowercase();
                if name.contains(&query) || description.contains(&query) || id.contains(&query) {
                    return true;
                }
                tool.descriptor.extensions.iter().any(|extension| {
                    extension.to_ascii_lowercase().contains(&query)
                        || format!(".{extension}")
                            .to_ascii_lowercase()
                            .contains(&query)
                })
            })
            .collect()
    }

    fn action_button(
        &self,
        label: String,
        action: ToolboxAction,
        disabled: bool,
        primary: bool,
    ) -> Box<dyn Element> {
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(if disabled {
                        theme::placeholder()
                    } else if primary {
                        theme::bg()
                    } else {
                        theme::accent_cool()
                    })
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                if !disabled {
                    ctx.dispatch_typed_action(action.clone());
                }
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(8.0)
        .with_padding_bottom(8.0)
        .with_background(if primary && !disabled {
            theme::accent()
        } else {
            theme::accent_cool_bg(if disabled { 8 } else { 24 })
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn category_chip(
        &self,
        label: &str,
        category: Option<ToolCategory>,
        selected: bool,
    ) -> Box<dyn Element> {
        let action = ToolboxAction::SetCategory(category);
        Container::new(
            EventHandler::new(
                ui_text::body(label.to_string(), self.font)
                    .with_color(if selected { theme::bg() } else { theme::text() })
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(if selected {
            theme::accent()
        } else {
            theme::accent_cool_bg(20)
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
        .with_border(Border::all(1.0).with_border_fill(if selected {
            theme::accent()
        } else {
            theme::border()
        }))
        .finish()
    }

    fn category_row(&self) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        let chips = [
            ("全部", None),
            ("文档", Some(ToolCategory::Documents)),
            ("表格", Some(ToolCategory::Spreadsheets)),
            ("3D", Some(ToolCategory::ThreeD)),
        ];
        for (index, (label, category)) in chips.into_iter().enumerate() {
            let selected = self.selected_category == category;
            let chip = self.category_chip(label, category, selected);
            row.add_child(
                Container::new(chip)
                    .with_margin_left(if index == 0 { 0.0 } else { 8.0 })
                    .finish(),
            );
        }
        row.finish()
    }

    fn search_box(&self) -> Box<dyn Element> {
        let field = render_search_field_with_caret(
            &self.search_query,
            &self.search_field.marked_text,
            "搜索工具名称或扩展名…",
            self.font,
            self.search_focused,
            false,
            self.caret_blink.visible,
            self.search_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(ToolboxAction::SearchEdit(action));
        })
        .focused(self.search_focused)
        .ime_preedit(!self.search_field.marked_text.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "escape" {
                ctx.dispatch_typed_action(ToolboxAction::BlurSearch);
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ToolboxAction::ActivateSearch);
        });

        let border_color = if self.search_focused {
            theme::accent_cool()
        } else {
            theme::border()
        };
        let row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::chat_sidebar_search_icon(theme::muted()))
                    .with_horizontal_margin(2.0)
                    .finish(),
            )
            .with_child(Expanded::new(1.0, input).finish())
            .finish();

        Container::new(row)
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(border_color))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish()
    }

    fn primary_action(&self, tool: &ToolSummary) -> (String, ToolboxAction, bool, bool) {
        if self.busy_tool.as_deref() == Some(tool.descriptor.id.as_str()) {
            return (
                "取消".into(),
                ToolboxAction::Cancel(tool.descriptor.id.clone()),
                false,
                false,
            );
        }
        match tool.status.stage {
            ToolInstallStage::Ready if tool.descriptor.requires_file => (
                "从文件打开".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                true,
            ),
            ToolInstallStage::Prepared if tool.descriptor.requires_file => (
                "到共享文件中打开".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                false,
            ),
            ToolInstallStage::Ready => (
                "打开".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                true,
            ),
            ToolInstallStage::UpdateAvailable => (
                "更新".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                true,
            ),
            ToolInstallStage::Failed if tool.status.available_version.is_some() => (
                "重试".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                true,
            ),
            ToolInstallStage::NotInstalled if tool.status.available_version.is_some() => (
                "安装".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                false,
                true,
            ),
            ToolInstallStage::Downloading
            | ToolInstallStage::Checking
            | ToolInstallStage::Verifying
            | ToolInstallStage::Installing => (
                "处理中".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                true,
                false,
            ),
            _ => (
                "未开放".into(),
                ToolboxAction::Primary(tool.descriptor.id.clone()),
                true,
                false,
            ),
        }
    }

    fn tool_status_tone(stage: &ToolInstallStage) -> StatusTone {
        match stage {
            ToolInstallStage::Ready => StatusTone::Success,
            ToolInstallStage::Prepared => StatusTone::Warn,
            ToolInstallStage::UpdateAvailable
            | ToolInstallStage::Downloading
            | ToolInstallStage::Checking
            | ToolInstallStage::Verifying
            | ToolInstallStage::Installing => StatusTone::Warn,
            ToolInstallStage::Failed => StatusTone::Danger,
            ToolInstallStage::Cancelled | ToolInstallStage::NotInstalled => StatusTone::Placeholder,
        }
    }

    fn install_badge(tool: &ToolSummary) -> (&'static str, StatusTone) {
        match tool.status.stage {
            ToolInstallStage::Ready | ToolInstallStage::Prepared => ("已安装", StatusTone::Success),
            ToolInstallStage::UpdateAvailable => ("可更新", StatusTone::Warn),
            ToolInstallStage::Downloading
            | ToolInstallStage::Checking
            | ToolInstallStage::Verifying
            | ToolInstallStage::Installing => ("安装中", StatusTone::Warn),
            ToolInstallStage::Failed => ("失败", StatusTone::Danger),
            ToolInstallStage::Cancelled => ("已取消", StatusTone::Placeholder),
            ToolInstallStage::NotInstalled if tool.status.available_version.is_some() => {
                ("未安装", StatusTone::Placeholder)
            }
            ToolInstallStage::NotInstalled => ("未开放", StatusTone::Placeholder),
        }
    }

    fn icon_opacity(tool: &ToolSummary) -> f32 {
        match tool.status.stage {
            ToolInstallStage::Ready | ToolInstallStage::Prepared => 1.0,
            ToolInstallStage::UpdateAvailable
            | ToolInstallStage::Downloading
            | ToolInstallStage::Checking
            | ToolInstallStage::Verifying
            | ToolInstallStage::Installing => 0.9,
            _ => 0.55,
        }
    }

    fn metadata_text(tool: &ToolSummary) -> String {
        let mut parts = Vec::new();
        match tool.descriptor.executor {
            ToolExecutorKind::LocalProcess => parts.push("本机工具".to_string()),
            ToolExecutorKind::SourceRuntime => parts.push("来源电脑运行器".to_string()),
            ToolExecutorKind::CloudGpu => parts.push("云 GPU".to_string()),
        }
        if let Some(package) = tool.descriptor.packages.first() {
            parts.push(match package.source {
                ToolSourceKind::WormholeR2 => "Wormhole 签名仓库".into(),
                ToolSourceKind::Official => "软件官方源".into(),
            });
        }
        if let Some(version) = tool
            .status
            .installed_version
            .as_ref()
            .or(tool.status.available_version.as_ref())
        {
            parts.push(format!("v{version}"));
        }
        if !tool.descriptor.extensions.is_empty() {
            parts.push(
                tool.descriptor
                    .extensions
                    .iter()
                    .map(|extension| format!(".{extension}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        parts.join(" · ")
    }

    fn badge_chip(&self, label: &str, tone: StatusTone) -> Box<dyn Element> {
        let fg = match tone {
            StatusTone::Success => theme::success(),
            StatusTone::Warn => theme::warn(),
            StatusTone::Danger => theme::danger(),
            StatusTone::Neutral | StatusTone::Muted | StatusTone::Placeholder => theme::muted(),
        };
        Container::new(
            ui_text::mono(label.to_string(), self.mono)
                .with_color(fg)
                .finish(),
        )
        .with_padding_left(8.0)
        .with_padding_right(8.0)
        .with_padding_top(3.0)
        .with_padding_bottom(3.0)
        .with_background(theme::accent_cool_bg(16))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
        .finish()
    }

    fn tool_card(&self, tool: &ToolSummary) -> Box<dyn Element> {
        let (badge_label, badge_tone) = Self::install_badge(tool);
        let title_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                Expanded::new(
                    1.0,
                    ui_text::body(tool.descriptor.name.clone(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Container::new(self.badge_chip(badge_label, badge_tone))
                    .with_margin_left(8.0)
                    .finish(),
            );

        let mut text = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Min);
        text.add_child(title_row.finish());
        text.add_child(
            Container::new(
                ui_text::body(tool.descriptor.description.clone(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(4.0)
            .finish(),
        );
        text.add_child(
            Container::new(
                ui_text::mono(Self::metadata_text(tool), self.mono)
                    .with_color(theme::placeholder())
                    .finish(),
            )
            .with_margin_top(7.0)
            .finish(),
        );
        let mut detail = tool.status.detail.clone();
        if tool.status.bytes_total > 0
            && matches!(
                tool.status.stage,
                ToolInstallStage::Downloading
                    | ToolInstallStage::Verifying
                    | ToolInstallStage::Installing
            )
        {
            let percent = tool
                .status
                .bytes_downloaded
                .saturating_mul(100)
                .checked_div(tool.status.bytes_total)
                .unwrap_or_default()
                .min(100);
            detail = format!("{detail} · {percent}%");
        }
        text.add_child(
            Container::new(status_line(
                detail,
                self.font,
                Self::tool_status_tone(&tool.status.stage),
            ))
            .with_margin_top(7.0)
            .finish(),
        );

        let icon = icons::tool_app_icon(
            tool.descriptor.icon.as_deref(),
            &tool.descriptor.id,
            Self::icon_opacity(tool),
        );

        let (label, action, disabled, primary) = self.primary_action(tool);
        let body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(icon)
            .with_child(
                Expanded::new(
                    1.0,
                    Container::new(text.finish())
                        .with_margin_left(14.0)
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                Container::new(self.action_button(label, action, disabled, primary))
                    .with_margin_left(18.0)
                    .finish(),
            )
            .finish();

        Container::new(ConstrainedBox::new(body).with_min_height(96.0).finish())
            .with_uniform_padding(16.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
            .finish()
    }

    fn start_install(&mut self, tool_id: String, ctx: &mut ViewContext<Self>) {
        self.busy_tool = Some(tool_id.clone());
        self.message = "正在准备下载；软件只会安装到当前用户的 Wormhole 目录。".into();
        self.message_tone = StatusTone::Warn;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                toolbox_install_tool(&state, tool_id).await
            },
            |view, output, ctx| {
                view.busy_tool = None;
                match output {
                    Ok(status) => {
                        view.message = status.detail;
                        view.message_tone = Self::tool_status_tone(&status.stage);
                    }
                    Err(error) => {
                        view.message = format!("安装失败: {error}");
                        view.message_tone = StatusTone::Danger;
                    }
                }
                view.refresh(ctx);
            },
        );
    }

    fn launch(&mut self, tool_id: String, ctx: &mut ViewContext<Self>) {
        let Some(tool) = self.tools.iter().find(|tool| tool.descriptor.id == tool_id) else {
            return;
        };
        if tool.descriptor.requires_file {
            self.message = format!(
                "请在同步盘或共享文件右侧面板选择支持的文件，再用 {} 打开。",
                tool.descriptor.name
            );
            self.message_tone = StatusTone::Neutral;
            ctx.notify();
            return;
        }
        self.busy_tool = Some(tool_id.clone());
        self.message = format!("正在打开 {}…", tool.descriptor.name);
        self.message_tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                toolbox_launch_tool(&state, tool_id).await
            },
            |view, output, ctx| {
                view.busy_tool = None;
                match output {
                    Ok(()) => {
                        view.message = "已启动。".into();
                        view.message_tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!("打开失败: {error}");
                        view.message_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn user_apps_section(&self) -> Box<dyn Element> {
        let mut section = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        section.add_child(
            Container::new(section_hint(
                "用户程序：导入 Linux 程序包（bin / 目录 / .AppImage / .deb）后可发布到控制面，并在共享文件「远程虚拟机打开」中选用。下载范围可选所有人 / 联系人 / 指定人。真正打开发生在 Linux worker；本机非 Linux 时可能无法写入 worker installed_apps，但不影响发布。",
                self.font,
            ))
            .with_margin_top(SECTION_GAP)
            .finish(),
        );
        let mut toolbar = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        toolbar.add_child(self.action_button(
            if self.user_apps_busy {
                "同步中…".into()
            } else {
                "同步用户程序".into()
            },
            ToolboxAction::RefreshUserApps,
            self.user_apps_busy,
            false,
        ));
        section.add_child(
            Container::new(toolbar.finish())
                .with_margin_top(8.0)
                .finish(),
        );
        section.add_child(
            Container::new(section_hint(
                "导入路径（文件 / 目录 / .AppImage / .deb）：",
                self.font,
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        let mut path_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        path_row.add_child(Expanded::new(1.0, self.import_path_box()).finish());
        path_row.add_child(
            Container::new(self.action_button(
                "浏览…".into(),
                ToolboxAction::BrowseImportPath,
                self.user_apps_busy,
                false,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        section.add_child(
            Container::new(path_row.finish())
                .with_margin_top(6.0)
                .finish(),
        );
        section.add_child(
            Container::new(section_hint("显示名称（可选）", self.font))
                .with_margin_top(8.0)
                .finish(),
        );
        section.add_child(
            Container::new(self.meta_field_box(
                &self.import_display_name,
                &self.import_display_name_field,
                "例如 My Viewer",
                self.import_display_name_focused,
                ToolboxAction::ImportDisplayNameEdit,
                ToolboxAction::ActivateImportDisplayName,
                ToolboxAction::BlurImportDisplayName,
            ))
            .with_margin_top(4.0)
            .finish(),
        );
        section.add_child(
            Container::new(section_hint(
                "关联扩展名（逗号分隔，留空=任意文件）",
                self.font,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        section.add_child(
            Container::new(self.meta_field_box(
                &self.import_extensions,
                &self.import_extensions_field,
                "例如 png, jpg, svg",
                self.import_extensions_focused,
                ToolboxAction::ImportExtensionsEdit,
                ToolboxAction::ActivateImportExtensions,
                ToolboxAction::BlurImportExtensions,
            ))
            .with_margin_top(4.0)
            .finish(),
        );
        section.add_child(
            Container::new(section_hint(
                "入口（目录相对路径 / deb 安装后二进制名；bin/AppImage 可留空）",
                self.font,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        section.add_child(
            Container::new(self.meta_field_box(
                &self.import_entrypoint,
                &self.import_entrypoint_field,
                "例如 bin/my-app 或 gimp",
                self.import_entrypoint_focused,
                ToolboxAction::ImportEntrypointEdit,
                ToolboxAction::ActivateImportEntrypoint,
                ToolboxAction::BlurImportEntrypoint,
            ))
            .with_margin_top(4.0)
            .finish(),
        );
        section.add_child(
            Container::new(section_hint("发布版本号", self.font))
                .with_margin_top(8.0)
                .finish(),
        );
        section.add_child(
            Container::new(self.meta_field_box(
                &self.publish_version,
                &self.publish_version_field,
                "1",
                self.publish_version_focused,
                ToolboxAction::ImportVersionEdit,
                ToolboxAction::ActivateImportVersion,
                ToolboxAction::BlurImportVersion,
            ))
            .with_margin_top(4.0)
            .finish(),
        );
        section.add_child(
            Container::new(section_hint(
                "指定人 user id（逗号分隔；发布·指定人 / 改权限·指定人时使用）",
                self.font,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        section.add_child(
            Container::new(self.meta_field_box(
                &self.allowlist_ids,
                &self.allowlist_ids_field,
                "uuid-1, uuid-2",
                self.allowlist_ids_focused,
                ToolboxAction::AllowlistIdsEdit,
                ToolboxAction::ActivateAllowlistIds,
                ToolboxAction::BlurAllowlistIds,
            ))
            .with_margin_top(4.0)
            .finish(),
        );
        section.add_child(
            Container::new(self.action_button(
                "导入到本机".into(),
                ToolboxAction::ImportLocal,
                self.user_apps_busy || self.import_path.trim().is_empty(),
                true,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if self.local_user_apps.is_empty() {
            section.add_child(
                Container::new(section_hint("本机尚未导入用户程序。", self.font))
                    .with_margin_top(10.0)
                    .finish(),
            );
        } else {
            for app in &self.local_user_apps {
                let mut row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);
                let ext_label = if app.extensions.is_empty() {
                    "任意扩展名".into()
                } else {
                    app.extensions.join(",")
                };
                row.add_child(
                    Expanded::new(
                        1.0,
                        status_line(
                            format!("{} ({}) · {}", app.display_name, app.app_id, ext_label),
                            self.font,
                            StatusTone::Neutral,
                        ),
                    )
                    .finish(),
                );
                row.add_child(self.action_button(
                    "发布·所有人".into(),
                    ToolboxAction::PublishLocal {
                        app_id: app.app_id.clone(),
                        visibility: "public".into(),
                    },
                    self.user_apps_busy,
                    false,
                ));
                row.add_child(self.action_button(
                    "发布·联系人".into(),
                    ToolboxAction::PublishLocal {
                        app_id: app.app_id.clone(),
                        visibility: "contacts".into(),
                    },
                    self.user_apps_busy,
                    false,
                ));
                row.add_child(self.action_button(
                    "发布·指定人".into(),
                    ToolboxAction::PublishLocal {
                        app_id: app.app_id.clone(),
                        visibility: "allowlist".into(),
                    },
                    self.user_apps_busy || self.allowlist_ids.trim().is_empty(),
                    false,
                ));
                row.add_child(self.action_button(
                    "删除".into(),
                    ToolboxAction::RemoveLocal(app.app_id.clone()),
                    self.user_apps_busy,
                    false,
                ));
                section.add_child(
                    Container::new(row.finish())
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
        }
        section.add_child(self.catalog_user_apps_list());
        section.finish()
    }

    fn catalog_user_apps_list(&self) -> Box<dyn Element> {
        let mut section = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        section.add_child(
            Container::new(section_hint(
                "可下载的用户程序目录（本机安装供 Linux worker 本地缓存；远程打开时目标 worker 也会按权限自动拉取）：",
                self.font,
            ))
            .with_margin_top(SECTION_GAP)
            .finish(),
        );
        if self.catalog_user_apps.is_empty() {
            section.add_child(
                Container::new(section_hint(
                    "目录为空，或当前账号无权查看（需登录）。",
                    self.font,
                ))
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for app in &self.catalog_user_apps {
                let mut row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);
                let ready = if app.package_ready {
                    "可安装"
                } else {
                    "待上传"
                };
                row.add_child(
                    Expanded::new(
                        1.0,
                        status_line(
                            format!(
                                "{} · {} · {} · {ready}",
                                app.display_name, app.visibility, app.version
                            ),
                            self.font,
                            StatusTone::Neutral,
                        ),
                    )
                    .finish(),
                );
                row.add_child(self.action_button(
                    "安装".into(),
                    ToolboxAction::InstallCatalog {
                        app_id: app.app_id.clone(),
                        version: app.version.clone(),
                    },
                    self.user_apps_busy || !app.package_ready,
                    true,
                ));
                row.add_child(self.action_button(
                    "改·所有人".into(),
                    ToolboxAction::UpdateCatalogAcl {
                        app_id: app.app_id.clone(),
                        version: app.version.clone(),
                        visibility: "public".into(),
                    },
                    self.user_apps_busy,
                    false,
                ));
                row.add_child(self.action_button(
                    "改·联系人".into(),
                    ToolboxAction::UpdateCatalogAcl {
                        app_id: app.app_id.clone(),
                        version: app.version.clone(),
                        visibility: "contacts".into(),
                    },
                    self.user_apps_busy,
                    false,
                ));
                row.add_child(self.action_button(
                    "改·指定人".into(),
                    ToolboxAction::UpdateCatalogAcl {
                        app_id: app.app_id.clone(),
                        version: app.version.clone(),
                        visibility: "allowlist".into(),
                    },
                    self.user_apps_busy || self.allowlist_ids.trim().is_empty(),
                    false,
                ));
                row.add_child(self.action_button(
                    "撤回".into(),
                    ToolboxAction::RevokeCatalog {
                        app_id: app.app_id.clone(),
                        version: app.version.clone(),
                    },
                    self.user_apps_busy,
                    false,
                ));
                section.add_child(
                    Container::new(row.finish())
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
        }
        section.finish()
    }

    fn import_path_box(&self) -> Box<dyn Element> {
        self.meta_field_box(
            &self.import_path,
            &self.import_path_field,
            "选择或粘贴本机路径",
            self.import_path_focused,
            ToolboxAction::ImportPathEdit,
            ToolboxAction::ActivateImportPath,
            ToolboxAction::BlurImportPath,
        )
    }

    fn meta_field_box(
        &self,
        value: &str,
        field: &TextFieldState,
        placeholder: &str,
        focused: bool,
        edit: impl Fn(TextFieldEditAction) -> ToolboxAction + Copy + 'static,
        activate: ToolboxAction,
        blur: ToolboxAction,
    ) -> Box<dyn Element> {
        let field_el = render_search_field_with_caret(
            value,
            &field.marked_text,
            placeholder,
            self.font,
            focused,
            false,
            self.caret_blink.visible,
            field.cursor,
        );
        let input = TextFieldInput::builder(field_el, move |ctx, action| {
            ctx.dispatch_typed_action(edit(action));
        })
        .focused(focused)
        .ime_preedit(!field.marked_text.is_empty())
        .on_keydown(move |ctx, keystroke| {
            if keystroke.key == "escape" {
                ctx.dispatch_typed_action(blur.clone());
                return DispatchEventResult::StopPropagation;
            }
            DispatchEventResult::PropagateToParent
        })
        .finish();
        wrap_text_field_focus_on_click(input, move |ctx| {
            ctx.dispatch_typed_action(activate.clone());
        })
    }
}

impl Entity for ToolboxView {
    type Event = ();
}

impl View for ToolboxView {
    fn ui_name() -> &'static str {
        "ToolboxView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let filtered = self.filtered_tools();
        let mut list = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Min);
        if self.tools.is_empty() {
            list.add_child(
                Container::new(section_hint(
                    if self.loading {
                        "正在加载工具目录…"
                    } else {
                        "工具目录为空。请检查签名目录地址后刷新。"
                    },
                    self.font,
                ))
                .with_uniform_padding(16.0)
                .finish(),
            );
        } else if filtered.is_empty() {
            list.add_child(
                Container::new(section_hint(
                    "没有匹配的工具。试试其他分类或清空搜索。",
                    self.font,
                ))
                .with_uniform_padding(16.0)
                .finish(),
            );
        } else {
            for (index, tool) in filtered.into_iter().enumerate() {
                let card = self.tool_card(tool);
                list.add_child(
                    Container::new(card)
                        .with_margin_top(if index == 0 {
                            SECTION_GAP
                        } else {
                            SECTION_GAP / 2.0
                        })
                        .finish(),
                );
            }
        }

        // Embedded under Settings → 虚拟机: page header is owned by SettingsView.
        let mut refresh_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        refresh_row.add_child(
            Expanded::new(1.0, Container::new(Empty::new().finish()).finish()).finish(),
        );
        refresh_row.add_child(self.action_button(
            if self.loading {
                "刷新中".into()
            } else {
                "刷新".into()
            },
            ToolboxAction::Refresh,
            self.loading,
            false,
        ));

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(refresh_row.finish());
        col.add_child(
            Container::new(section_hint(
                "软件包按需下载并验签，在来源电脑的 Ubuntu 隔离运行器中使用；文件不上传云端。",
                self.font,
            ))
            .with_margin_top(6.0)
            .finish(),
        );
        col.add_child(
            Container::new(self.search_box())
                .with_margin_top(12.0)
                .finish(),
        );
        col.add_child(
            Container::new(self.category_row())
                .with_margin_top(12.0)
                .finish(),
        );
        if !self.message.is_empty() {
            col.add_child(
                Container::new(status_line(
                    self.message.clone(),
                    self.font,
                    self.message_tone,
                ))
                .with_margin_top(10.0)
                .finish(),
            );
        }
        col.add_child(list.finish());
        col.add_child(self.user_apps_section());
        col.finish()
    }
}

impl TypedActionView for ToolboxView {
    type Action = ToolboxAction;

    fn handle_action(&mut self, action: &ToolboxAction, ctx: &mut ViewContext<Self>) {
        match action {
            ToolboxAction::Refresh => self.refresh(ctx),
            ToolboxAction::SetCategory(category) => {
                self.selected_category = *category;
                ctx.notify();
            }
            ToolboxAction::ActivateSearch => {
                self.search_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurSearch => {
                self.search_focused = false;
                self.search_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::SearchEdit(edit) => {
                self.search_field.apply(&mut self.search_query, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::Primary(tool_id) => {
                let Some(tool) = self
                    .tools
                    .iter()
                    .find(|tool| tool.descriptor.id == *tool_id)
                else {
                    return;
                };
                match tool.status.stage {
                    ToolInstallStage::Ready | ToolInstallStage::Prepared => {
                        self.launch(tool_id.clone(), ctx)
                    }
                    ToolInstallStage::NotInstalled
                    | ToolInstallStage::UpdateAvailable
                    | ToolInstallStage::Failed => self.start_install(tool_id.clone(), ctx),
                    _ => {}
                }
            }
            ToolboxAction::Cancel(tool_id) => {
                match toolbox_cancel_install(tool_id.clone()) {
                    Ok(()) => {
                        self.message = "正在取消；已下载的数据会保留以便下次继续。".into();
                        self.message_tone = StatusTone::Warn;
                    }
                    Err(error) => {
                        self.message = format!("取消失败: {error}");
                        self.message_tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            }
            ToolboxAction::RefreshUserApps => self.refresh_user_apps(ctx),
            ToolboxAction::ActivateImportPath => {
                self.clear_user_app_field_focus();
                self.import_path_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurImportPath => {
                self.import_path_focused = false;
                self.import_path_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ImportPathEdit(edit) => {
                self.import_path_field.apply(&mut self.import_path, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ActivateImportDisplayName => {
                self.clear_user_app_field_focus();
                self.import_display_name_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurImportDisplayName => {
                self.import_display_name_focused = false;
                self.import_display_name_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ImportDisplayNameEdit(edit) => {
                self.import_display_name_field
                    .apply(&mut self.import_display_name, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ActivateImportExtensions => {
                self.clear_user_app_field_focus();
                self.import_extensions_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurImportExtensions => {
                self.import_extensions_focused = false;
                self.import_extensions_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ImportExtensionsEdit(edit) => {
                self.import_extensions_field
                    .apply(&mut self.import_extensions, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ActivateImportEntrypoint => {
                self.clear_user_app_field_focus();
                self.import_entrypoint_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurImportEntrypoint => {
                self.import_entrypoint_focused = false;
                self.import_entrypoint_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ImportEntrypointEdit(edit) => {
                self.import_entrypoint_field
                    .apply(&mut self.import_entrypoint, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ActivateImportVersion => {
                self.clear_user_app_field_focus();
                self.publish_version_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurImportVersion => {
                self.publish_version_focused = false;
                self.publish_version_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ImportVersionEdit(edit) => {
                self.publish_version_field
                    .apply(&mut self.publish_version, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::ActivateAllowlistIds => {
                self.clear_user_app_field_focus();
                self.allowlist_ids_focused = true;
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BlurAllowlistIds => {
                self.allowlist_ids_focused = false;
                self.allowlist_ids_field.clear_marked();
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::AllowlistIdsEdit(edit) => {
                self.allowlist_ids_field.apply(&mut self.allowlist_ids, edit);
                sync_caret_blink(self, ctx);
                ctx.notify();
            }
            ToolboxAction::BrowseImportPath => {
                let picked = rfd::FileDialog::new()
                    .set_title("选择要导入的 Linux 程序")
                    .pick_file()
                    .or_else(|| {
                        rfd::FileDialog::new()
                            .set_title("或选择程序目录")
                            .pick_folder()
                    });
                if let Some(path) = picked {
                    self.import_path = path.display().to_string();
                    self.import_path_field = TextFieldState::new();
                    ctx.notify();
                }
            }
            ToolboxAction::ImportLocal => {
                let path = self.import_path.trim().to_string();
                if path.is_empty() {
                    return;
                }
                let display_name = {
                    let t = self.import_display_name.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                };
                let entrypoint = {
                    let t = self.import_entrypoint.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                };
                let extensions = parse_extension_list(&self.import_extensions);
                self.user_apps_busy = true;
                self.message = "正在导入用户程序…".into();
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_import(
                            &state,
                            WorkspaceUserAppImportParams {
                                path,
                                display_name,
                                app_id: None,
                                entrypoint,
                                extensions,
                                promote: true,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(result) => {
                                if let Some(warning) = result.promote_warning {
                                    view.message = format!(
                                        "已导入 {}，但未写入 worker installed_apps：{warning}",
                                        result.manifest.display_name
                                    );
                                    view.message_tone = StatusTone::Warn;
                                } else {
                                    view.message =
                                        format!("已导入 {}", result.manifest.display_name);
                                    view.message_tone = StatusTone::Success;
                                }
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("导入失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            ToolboxAction::RemoveLocal(app_id) => {
                let app_id = app_id.clone();
                self.user_apps_busy = true;
                self.message = format!("正在删除 {app_id}…");
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_remove(
                            &state,
                            WorkspaceUserAppIdParams { app_id },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(()) => {
                                view.message = "已删除本机用户程序".into();
                                view.message_tone = StatusTone::Success;
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("删除失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            ToolboxAction::PublishLocal { app_id, visibility } => {
                let app_id = app_id.clone();
                let visibility = visibility.clone();
                let version = {
                    let t = self.publish_version.trim();
                    if t.is_empty() {
                        "1".into()
                    } else {
                        t.to_string()
                    }
                };
                let allowed_user_ids = if visibility == "allowlist" {
                    self.allowlist_ids
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                } else {
                    Vec::new()
                };
                self.user_apps_busy = true;
                self.message = format!("正在发布 {app_id}…");
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_publish(
                            &state,
                            WorkspaceUserAppPublishParams {
                                app_id,
                                version,
                                visibility,
                                allowed_user_ids,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(app) => {
                                view.message = format!(
                                    "已发布 {}（{} · v{}）",
                                    app.display_name, app.visibility, app.version
                                );
                                view.message_tone = StatusTone::Success;
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("发布失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            ToolboxAction::InstallCatalog { app_id, version } => {
                let app_id = app_id.clone();
                let version = version.clone();
                self.user_apps_busy = true;
                self.message = format!("正在安装 {app_id}（可能需要数分钟）…");
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_catalog_install(
                            &state,
                            WorkspaceUserAppCatalogInstallParams {
                                app_id,
                                version,
                                promote: true,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(result) => {
                                if let Some(warning) = result.promote_warning {
                                    view.message = format!(
                                        "已安装 {}，但未写入 worker installed_apps：{warning}",
                                        result.manifest.display_name
                                    );
                                    view.message_tone = StatusTone::Warn;
                                } else {
                                    view.message =
                                        format!("已安装 {}", result.manifest.display_name);
                                    view.message_tone = StatusTone::Success;
                                }
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("安装失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            ToolboxAction::UpdateCatalogAcl {
                app_id,
                version,
                visibility,
            } => {
                let app_id = app_id.clone();
                let version = version.clone();
                let visibility = visibility.clone();
                let allowed_user_ids = if visibility == "allowlist" {
                    self.allowlist_ids
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                } else {
                    Vec::new()
                };
                self.user_apps_busy = true;
                self.message = format!("正在更新 {app_id}@{version} 下载权限…");
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_catalog_update_acl(
                            &state,
                            WorkspaceUserAppCatalogUpdateAclParams {
                                app_id,
                                version,
                                visibility,
                                allowed_user_ids,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(app) => {
                                view.message = format!(
                                    "已更新 {} 下载权限为 {}",
                                    app.display_name, app.visibility
                                );
                                view.message_tone = StatusTone::Success;
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("更新权限失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            ToolboxAction::RevokeCatalog { app_id, version } => {
                let app_id = app_id.clone();
                let version = version.clone();
                self.user_apps_busy = true;
                self.message = format!("正在撤回 {app_id}@{version}…");
                self.message_tone = StatusTone::Placeholder;
                ctx.notify();
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        workspace_user_app_catalog_revoke(
                            &state,
                            WorkspaceUserAppCatalogRevokeParams { app_id, version },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.user_apps_busy = false;
                        match output {
                            Ok(()) => {
                                view.message = "已从目录撤回该版本".into();
                                view.message_tone = StatusTone::Success;
                                view.refresh_user_apps(ctx);
                            }
                            Err(error) => {
                                view.message = format!("撤回失败: {error}");
                                view.message_tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
        }
    }
}

impl ToolboxView {
    fn clear_user_app_field_focus(&mut self) {
        self.search_focused = false;
        self.import_path_focused = false;
        self.import_display_name_focused = false;
        self.import_extensions_focused = false;
        self.import_entrypoint_focused = false;
        self.publish_version_focused = false;
        self.allowlist_ids_focused = false;
    }
}

impl CaretBlinkHost for ToolboxView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
            || self.import_path_focused
            || self.import_display_name_focused
            || self.import_extensions_focused
            || self.import_entrypoint_focused
            || self.publish_version_focused
            || self.allowlist_ids_focused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wormhole_desktop_core::toolbox_ui::{ToolDescriptor, ToolInstallStatus};

    fn sample_tool(id: &str, categories: Vec<ToolCategory>, extensions: &[&str]) -> ToolSummary {
        ToolSummary {
            descriptor: ToolDescriptor {
                id: id.into(),
                name: id.to_ascii_uppercase(),
                description: format!("{id} tool"),
                executor: ToolExecutorKind::SourceRuntime,
                categories,
                icon: None,
                extensions: extensions.iter().map(|ext| (*ext).into()).collect(),
                requires_file: true,
                packages: Vec::new(),
            },
            status: ToolInstallStatus {
                tool_id: id.into(),
                stage: ToolInstallStage::NotInstalled,
                detail: String::new(),
                bytes_downloaded: 0,
                bytes_total: 0,
                installed_version: None,
                available_version: None,
                entrypoint: None,
                last_error: None,
                updated_at: 0,
            },
        }
    }

    #[test]
    fn install_badge_distinguishes_ready_and_unavailable() {
        let mut ready = sample_tool("onlyoffice", vec![ToolCategory::Documents], &["docx"]);
        ready.status.stage = ToolInstallStage::Ready;
        assert_eq!(ToolboxView::install_badge(&ready).0, "已安装");

        let mut available = sample_tool("onlyoffice", vec![ToolCategory::Documents], &["docx"]);
        available.status.available_version = Some("1.0.0".into());
        assert_eq!(ToolboxView::install_badge(&available).0, "未安装");

        let unavailable = sample_tool("blender", vec![ToolCategory::ThreeD], &["blend"]);
        assert_eq!(ToolboxView::install_badge(&unavailable).0, "未开放");
    }
}
