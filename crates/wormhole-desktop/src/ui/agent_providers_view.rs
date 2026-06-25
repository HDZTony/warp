use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_deeplink::DeepLinkImportPreviewDto;
use wormhole_desktop_core::agent_provider_commands;
use wormhole_desktop_core::agent_provider_store::AgentProviderSummaryDto;

use crate::ui::codex_provider_import_model::{
    close_preview, open_preview, set_importing, set_message, snapshot_importing, snapshot_message,
    snapshot_preview, SharedCodexProviderImportModel,
};
use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, truncate_middle, StatusTone,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum AgentProvidersAction {
    Refresh,
    Activate(String),
    Delete(String),
    QueryUsage(String),
    ConfirmImport,
    CancelImport,
    ParseClipboard,
}

pub struct AgentProvidersView {
    core: CoreHandle,
    font: FamilyId,
    import_model: SharedCodexProviderImportModel,
    providers: Vec<AgentProviderSummaryDto>,
    active_provider_id: Option<String>,
    llm_summary: String,
    status: String,
}

impl AgentProvidersView {
    pub fn new(
        ctx: &mut ViewContext<Self>,
        core: CoreHandle,
        import_model: SharedCodexProviderImportModel,
    ) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            import_model,
            providers: Vec::new(),
            active_provider_id: None,
            llm_summary: "加载 Codex 上游…".into(),
            status: String::new(),
        };
        view.refresh(ctx);
        view
    }

    pub fn open_deeplink_url(&mut self, url: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                wormhole_desktop_core::agent_provider_commands::parse_agent_deeplink(url).await
            },
            |view, output, ctx| {
                match output {
                    Ok(preview) => {
                        open_preview(&view.import_model, preview);
                        view.status = "已解析供应商链接，请确认导入".into();
                    }
                    Err(err) => view.status = err,
                }
                ctx.notify();
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let providers = agent_provider_commands::agent_providers_list(&state).await;
                let llm = wormhole_desktop_core::agent_llm_commands::agent_llm_config(&state).await;
                (providers, llm)
            },
            |view, output, ctx| {
                let (providers, llm) = output;
                if let Ok(list) = providers {
                    view.active_provider_id = list.active_provider_id.clone();
                    view.providers = list.providers;
                }
                view.llm_summary = match llm {
                    Ok(cfg) => format!(
                        "当前：{} · 模型 {} · {}",
                        cfg.provider, cfg.model, cfg.base_url
                    ),
                    Err(err) => format!("LLM 配置错误: {err}"),
                };
                ctx.notify();
            },
        );
    }

    fn confirm_import(&mut self, preview: DeepLinkImportPreviewDto, ctx: &mut ViewContext<Self>) {
        set_importing(&self.import_model, true);
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::import_agent_provider(&state, preview.request).await
            },
            move |view, output, ctx| {
                set_importing(&view.import_model, false);
                match output {
                    Ok(result) => {
                        close_preview(&view.import_model);
                        view.status = format!("已导入供应商 {}", result.provider_id);
                        view.refresh(ctx);
                    }
                    Err(err) => set_message(&view.import_model, err),
                }
                ctx.notify();
            },
        );
    }

    fn activate(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::activate_agent_provider(&state, id).await
            },
            |view, output, ctx| {
                view.status = match output {
                    Ok(_) => "已切换供应商".into(),
                    Err(err) => err,
                };
                view.refresh(ctx);
            },
        );
    }

    fn delete(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::delete_agent_provider(&state, id).await
            },
            |view, output, ctx| {
                view.status = match output {
                    Ok(_) => "已删除供应商".into(),
                    Err(err) => err,
                };
                view.refresh(ctx);
            },
        );
    }

    fn query_usage(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::query_agent_provider_usage(&state, id).await
            },
            |view, output, ctx| {
                view.status = match output {
                    Ok(result) => format_usage_result(&result),
                    Err(err) => err,
                };
                ctx.notify();
            },
        );
    }

    fn parse_clipboard(&mut self, ctx: &mut ViewContext<Self>) {
        let text = read_clipboard_text().unwrap_or_default();
        let url = text
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("ccswitch://"))
            .map(str::to_string);
        match url {
            Some(url) => self.open_deeplink_url(url, ctx),
            None => self.status = "剪贴板中未找到 ccswitch:// 链接".into(),
        }
    }

    fn action_button(&self, label: &str, action: AgentProvidersAction) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::accent())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(28))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn import_card(&self, preview: &DeepLinkImportPreviewDto) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(ui_text::title("确认导入 Codex 供应商", self.font).finish());
        col.add_child(
            ui_text::body(
                "请核对信息。导入后可在列表中点击「查询用量」执行 CC Switch 同款脚本。",
                self.font,
            )
            .with_color(theme::muted())
            .finish(),
        );
        for (label, value) in [
            ("应用", preview.app.as_str()),
            ("名称", preview.name.as_str()),
            ("官网", preview.homepage.as_str()),
            ("端点", preview.endpoint.as_str()),
            (
                "API Key",
                preview.api_key_masked.as_deref().unwrap_or("****"),
            ),
            ("模型", preview.model.as_str()),
        ] {
            col.add_child(
                ui_text::mono(format!("{label}: {value}"), self.font)
                    .with_color(theme::text())
                    .finish(),
            );
        }
        let usage = if preview.usage_enabled {
            match preview.usage_auto_interval {
                Some(minutes) => format!("用量查询：已启用 · 每 {minutes} 分钟"),
                None => "用量查询：已启用".into(),
            }
        } else {
            "用量查询：未启用".into()
        };
        col.add_child(ui_text::body(usage, self.font).finish());
        col.add_child(
            ui_text::body(
                if preview.enabled {
                    "导入后启用：是（enabled=true）"
                } else {
                    "导入后启用：否"
                },
                self.font,
            )
            .finish(),
        );
        if !snapshot_message(&self.import_model).is_empty() {
            col.add_child(
                ui_text::body(snapshot_message(&self.import_model), self.font)
                    .with_color(theme::danger())
                    .finish(),
            );
        }
        let importing = snapshot_importing(&self.import_model);
        let mut actions = Flex::row();
        actions.add_child(self.action_button(
            if importing { "导入中…" } else { "导入" },
            AgentProvidersAction::ConfirmImport,
        ));
        actions.add_child(self.action_button("取消", AgentProvidersAction::CancelImport));
        col.add_child(actions.finish());
        Container::new(col.finish())
            .with_uniform_padding(12.0)
            .with_background(theme::accent_bg(18))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::accent()))
            .finish()
    }
}

impl Entity for AgentProvidersView {
    type Event = ();
}

impl View for AgentProvidersView {
    fn ui_name() -> &'static str {
        "AgentProvidersView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("Codex 供应商", self.font));
        col.add_child(
            section_hint(
                "兼容 CC Switch 的 ccswitch:// 深度链接。若系统默认处理程序不是 Wormhole，可从剪贴板导入。",
                self.font,
            ),
        );
        col.add_child(
            status_line(self.llm_summary.clone(), self.font, StatusTone::Muted),
        );

        let mut toolbar = Flex::row();
        toolbar.add_child(self.action_button("刷新", AgentProvidersAction::Refresh));
        toolbar.add_child(self.action_button("从剪贴板导入", AgentProvidersAction::ParseClipboard));
        col.add_child(toolbar.finish());

        if let Some(preview) = snapshot_preview(&self.import_model) {
            col.add_child(self.import_card(&preview));
        }

        if !self.status.is_empty() {
            let tone = if self.status.contains("错误")
                || self.status.contains("失败")
                || self.status.contains("未找到")
            {
                StatusTone::Danger
            } else if self.status.contains("已") {
                StatusTone::Success
            } else {
                StatusTone::Neutral
            };
            col.add_child(status_line(self.status.clone(), self.font, tone));
        }

        col.add_child(section_title("已导入供应商", self.font));
        if self.providers.is_empty() {
            col.add_child(status_line(
                "暂无自定义供应商。使用「从剪贴板导入」或打开 ccswitch:// 链接添加。",
                self.font,
                StatusTone::Placeholder,
            ));
        } else {
            for provider in &self.providers {
                let active = provider.is_active;
                let line = truncate_middle(
                    &format!(
                        "{}{} · {} · {}{}",
                        provider.name,
                        if active { " [当前]" } else { "" },
                        provider.model,
                        provider.base_url,
                        if provider.chat_completions_upstream {
                            " · Chat 代理"
                        } else {
                            " · Responses 直连"
                        }
                    ),
                    120,
                );
                col.add_child(ui_text::mono(line, self.font).finish());
                let mut row = Flex::row();
                if !active {
                    let id = provider.id.clone();
                    row.add_child(self.action_button("启用", AgentProvidersAction::Activate(id)));
                }
                if provider.usage_enabled {
                    let usage_id = provider.id.clone();
                    row.add_child(
                        self.action_button("查询用量", AgentProvidersAction::QueryUsage(usage_id)),
                    );
                }
                let delete_id = provider.id.clone();
                row.add_child(self.action_button("删除", AgentProvidersAction::Delete(delete_id)));
                col.add_child(row.finish());
            }
        }

        section_card(col.finish())
    }
}

fn format_usage_result(result: &agent_provider_commands::AgentUsageResultDto) -> String {
    if !result.success {
        return result
            .error
            .clone()
            .unwrap_or_else(|| "用量查询失败".into());
    }
    let Some(items) = result.data.as_ref().filter(|items| !items.is_empty()) else {
        return "用量查询成功，但无数据".into();
    };
    let first = &items[0];
    if first.is_valid == Some(false) {
        return first
            .invalid_message
            .clone()
            .unwrap_or_else(|| "用量无效".into());
    }
    let mut parts = Vec::new();
    if let Some(plan) = &first.plan_name {
        parts.push(plan.clone());
    }
    if let (Some(remaining), Some(unit)) = (first.remaining, &first.unit) {
        parts.push(format!("剩余 {remaining} {unit}"));
    } else if let (Some(used), Some(total)) = (first.used, first.total) {
        parts.push(format!("已用 {used} / {total}"));
    }
    if let Some(extra) = &first.extra {
        parts.push(extra.clone());
    }
    if parts.is_empty() {
        "用量查询成功".into()
    } else {
        parts.join(" · ")
    }
}

impl TypedActionView for AgentProvidersView {
    type Action = AgentProvidersAction;

    fn handle_action(&mut self, action: &AgentProvidersAction, ctx: &mut ViewContext<Self>) {
        match action {
            AgentProvidersAction::Refresh => self.refresh(ctx),
            AgentProvidersAction::Activate(id) => self.activate(id.clone(), ctx),
            AgentProvidersAction::Delete(id) => self.delete(id.clone(), ctx),
            AgentProvidersAction::QueryUsage(id) => self.query_usage(id.clone(), ctx),
            AgentProvidersAction::ParseClipboard => self.parse_clipboard(ctx),
            AgentProvidersAction::CancelImport => {
                close_preview(&self.import_model);
                ctx.notify();
            }
            AgentProvidersAction::ConfirmImport => {
                if let Some(preview) = snapshot_preview(&self.import_model) {
                    self.confirm_import(preview, ctx);
                }
            }
        }
    }
}

#[cfg(windows)]
fn read_clipboard_text() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 {
            return None;
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT as u32);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = OsString::from_wide(slice).to_string_lossy().into_owned();
        GlobalUnlock(handle);
        CloseClipboard();
        Some(text)
    }
}

#[cfg(not(windows))]
fn read_clipboard_text() -> Option<String> {
    None
}
