use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::toolbox_ui::{
    toolbox_cancel_install, toolbox_install_tool, toolbox_launch_tool, toolbox_list_tools,
    ToolCategory, ToolExecutorKind, ToolInstallStage, ToolSourceKind, ToolSummary,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, view_panel, StatusTone, HUD_RADIUS, SECTION_GAP,
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
}

pub struct ToolboxView {
    core: CoreHandle,
    #[allow(dead_code)]
    coordinator: std::sync::Arc<std::sync::Mutex<crate::coordinator::CoordinatorState>>,
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
}

impl ToolboxView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        coordinator: std::sync::Arc<std::sync::Mutex<crate::coordinator::CoordinatorState>>,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mono = crate::ui::fonts::load_mono_font(ctx, font);
        let mut view = Self {
            core,
            coordinator,
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
        };
        view.refresh(ctx);
        view
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

        let mut heading = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(Expanded::new(1.0, section_title("工具箱", self.font)).finish());
        heading.add_child(self.action_button(
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
        col.add_child(heading.finish());
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
        view_panel(col.finish())
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
        }
    }
}

impl CaretBlinkHost for ToolboxView {
    fn caret_blink(&mut self) -> &mut CaretBlink {
        &mut self.caret_blink
    }

    fn caret_input_focused(&self) -> bool {
        self.search_focused
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
