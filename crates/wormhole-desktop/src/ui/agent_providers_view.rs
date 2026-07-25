use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_codex_presets::{self, CodexProviderPreset};
use wormhole_desktop_core::agent_llm_commands::{
    self, ConfigureAgentLlmParams, TestAgentLlmConnectionParams,
};
use wormhole_desktop_core::agent_provider_commands;
use wormhole_desktop_core::agent_provider_store::AgentProviderSummaryDto;

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_hint, section_title, status_line, truncate_middle, StatusTone,
};
use crate::ui::text_field_input::{
    render_field_with_caret, wrap_text_field_focus_on_click, TextFieldEditAction, TextFieldInput,
    TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum AgentProvidersAction {
    Refresh,
    Activate(String),
    Delete(String),
    QueryUsage(String),
    CycleByokPreset,
    CycleByokModel,
    FocusApiKey,
    ApiKeyEdit(TextFieldEditAction),
    SaveByok,
    TestByok,
}

pub struct AgentProvidersView {
    core: CoreHandle,
    font: FamilyId,
    providers: Vec<AgentProviderSummaryDto>,
    active_provider_id: Option<String>,
    llm_summary: String,
    status: String,
    busy: bool,
    byok_presets: Vec<CodexProviderPreset>,
    byok_preset_index: usize,
    byok_model_index: usize,
    api_key_draft: String,
    api_key_field: TextFieldState,
    api_key_focused: bool,
}

fn byok_presets() -> Vec<CodexProviderPreset> {
    agent_codex_presets::list_presets()
        .into_iter()
        .filter(|p| p.id != "control_plane")
        .collect()
}

impl AgentProvidersView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let presets = byok_presets();
        let mut view = Self {
            core,
            font,
            providers: Vec::new(),
            active_provider_id: None,
            llm_summary: "加载 Codex 上游…".into(),
            status: String::new(),
            busy: false,
            byok_presets: presets,
            byok_preset_index: 0,
            byok_model_index: 0,
            api_key_draft: String::new(),
            api_key_field: TextFieldState::new(),
            api_key_focused: false,
        };
        view.refresh(ctx);
        view
    }

    fn current_byok_preset(&self) -> Option<&CodexProviderPreset> {
        self.byok_presets.get(self.byok_preset_index)
    }

    fn current_byok_model_id(&self) -> String {
        let Some(preset) = self.current_byok_preset() else {
            return String::new();
        };
        preset
            .models
            .get(self.byok_model_index)
            .map(|m| m.id.clone())
            .unwrap_or_else(|| preset.default_model.clone())
    }

    fn cycle_byok_preset(&mut self, ctx: &mut ViewContext<Self>) {
        if self.byok_presets.is_empty() {
            return;
        }
        self.byok_preset_index = (self.byok_preset_index + 1) % self.byok_presets.len();
        self.byok_model_index = 0;
        self.api_key_draft.clear();
        self.api_key_field = TextFieldState::new();
        if let Some(preset) = self.current_byok_preset() {
            if let Some(existing) = self.providers.iter().find(|p| p.id == preset.id) {
                // Keep draft empty; hint shown via list. User re-enters to rotate key.
                let _ = existing;
            }
        }
        ctx.notify();
    }

    fn cycle_byok_model(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_byok_preset() else {
            return;
        };
        if preset.models.is_empty() {
            return;
        }
        self.byok_model_index = (self.byok_model_index + 1) % preset.models.len();
        ctx.notify();
    }

    fn save_byok(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_byok_preset().cloned() else {
            self.status = "没有可用的上游预设".into();
            ctx.notify();
            return;
        };
        let api_key = self.api_key_draft.trim().to_string();
        if api_key.is_empty() {
            self.status = "请填写 API Key".into();
            ctx.notify();
            return;
        }
        let model = self.current_byok_model_id();
        self.busy = true;
        self.status = format!("正在保存 {}…", preset.name);
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_llm_commands::configure_agent_llm(
                    &state,
                    ConfigureAgentLlmParams {
                        provider: Some(preset.id),
                        api_key: Some(api_key),
                        model: Some(model),
                        base_url: Some(preset.default_base_url),
                    },
                )
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(cfg) => {
                        view.status = format!(
                            "已保存自备 Key：{} · {}（可在 Agent 模型菜单切换）",
                            cfg.provider, cfg.model
                        );
                        view.api_key_draft.clear();
                        view.api_key_field = TextFieldState::new();
                        view.api_key_focused = false;
                    }
                    Err(err) => {
                        view.status = err;
                    }
                }
                view.refresh(ctx);
            },
        );
    }

    fn test_byok(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_byok_preset().cloned() else {
            self.status = "没有可用的上游预设".into();
            ctx.notify();
            return;
        };
        let api_key = self.api_key_draft.trim().to_string();
        self.busy = true;
        self.status = format!("正在测试 {}…", preset.name);
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_llm_commands::test_agent_llm_connection(
                    &state,
                    Some(TestAgentLlmConnectionParams {
                        provider: Some(preset.id),
                        api_key: if api_key.is_empty() {
                            None
                        } else {
                            Some(api_key)
                        },
                        base_url: Some(preset.default_base_url),
                    }),
                )
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                view.status = match output {
                    Ok(result) => result.message,
                    Err(err) => err,
                };
                ctx.notify();
            },
        );
    }

    fn byok_api_key_field(&self) -> Box<dyn Element> {
        let field = render_field_with_caret(
            &self.api_key_draft,
            &self.api_key_field.marked_text,
            "粘贴 API Key（sk-…）",
            self.font,
            self.api_key_focused,
            false,
            true,
            self.api_key_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(AgentProvidersAction::ApiKeyEdit(action));
        })
        .focused(self.api_key_focused)
        .ime_preedit(!self.api_key_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(AgentProvidersAction::FocusApiKey);
        });
        Container::new(input)
            .with_uniform_padding(10.0)
            .with_vertical_margin(6.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(if self.api_key_focused {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish()
    }

    fn byok_form_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("自备 API Key", self.font));
        col.add_child(section_hint(
            "默认使用 Key Pool（含 GPT）。填写自备 Key 后，对应模型会出现在 Agent 面板的模型菜单中。",
            self.font,
        ));

        let preset_label = self
            .current_byok_preset()
            .map(|p| format!("{} · {}", p.name, p.default_base_url))
            .unwrap_or_else(|| "无预设".into());
        let model_label = self
            .current_byok_preset()
            .and_then(|p| p.models.get(self.byok_model_index))
            .map(|m| m.label.clone())
            .unwrap_or_else(|| self.current_byok_model_id());

        col.add_child(status_line(
            format!("上游：{preset_label}"),
            self.font,
            StatusTone::Muted,
        ));
        col.add_child(status_line(
            format!("模型：{model_label}"),
            self.font,
            StatusTone::Muted,
        ));

        let mut cycle_row = Flex::row();
        cycle_row.add_child(self.action_button("切换上游", AgentProvidersAction::CycleByokPreset));
        cycle_row.add_child(
            Container::new(self.action_button("切换模型", AgentProvidersAction::CycleByokModel))
                .with_margin_left(8.0)
                .finish(),
        );
        col.add_child(cycle_row.finish());
        col.add_child(self.byok_api_key_field());

        let mut save_row = Flex::row();
        save_row.add_child(self.action_button("保存 Key", AgentProvidersAction::SaveByok));
        save_row.add_child(
            Container::new(self.action_button("测试连接", AgentProvidersAction::TestByok))
                .with_margin_left(8.0)
                .finish(),
        );
        col.add_child(save_row.finish());
        col.finish()
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在刷新供应商…".into();
        ctx.notify();
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
                match providers {
                    Ok(list) => {
                        view.active_provider_id = list.active_provider_id.clone();
                        view.providers = list.providers;
                        view.status = "供应商列表已刷新".into();
                    }
                    Err(error) => {
                        view.status = format!("刷新供应商失败: {error}");
                    }
                }
                view.llm_summary = match llm {
                    Ok(cfg) => format!(
                        "当前：{} · 模型 {} · {}",
                        cfg.provider, cfg.model, cfg.base_url
                    ),
                    Err(err) => format!("LLM 配置错误: {err}"),
                };
                view.busy = false;
                ctx.notify();
            },
        );
    }

    fn activate(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在启用供应商…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::activate_agent_provider(&state, id).await
            },
            |view, output, ctx| {
                view.busy = false;
                view.status = match output {
                    Ok(_) => "已切换供应商".into(),
                    Err(err) => err,
                };
                view.refresh(ctx);
            },
        );
    }

    fn delete(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在删除供应商…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::delete_agent_provider(&state, id).await
            },
            |view, output, ctx| {
                view.busy = false;
                view.status = match output {
                    Ok(_) => "已删除供应商".into(),
                    Err(err) => err,
                };
                view.refresh(ctx);
            },
        );
    }

    fn query_usage(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在查询用量…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_provider_commands::query_agent_provider_usage(&state, id).await
            },
            |view, output, ctx| {
                view.busy = false;
                view.status = match output {
                    Ok(result) => format_usage_result(&result),
                    Err(err) => err,
                };
                ctx.notify();
            },
        );
    }

    fn action_button(&self, label: &str, action: AgentProvidersAction) -> Box<dyn Element> {
        let label = label.to_string();
        let disabled = self.busy;
        Container::new(
            EventHandler::new(
                ui_text::body(label, self.font)
                    .with_color(theme::accent())
                    .finish(),
            )
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
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
        col.add_child(section_title("AGENT · Codex 供应商", self.font));
        col.add_child(section_hint(
            "默认使用 control plane Key Pool（含 GPT）。自备 Key 后可在 Agent 模型菜单切换；不拦截 ccswitch://。",
            self.font,
        ));
        col.add_child(status_line(
            self.llm_summary.clone(),
            self.font,
            StatusTone::Muted,
        ));

        let mut toolbar = Flex::row();
        toolbar.add_child(self.action_button("刷新", AgentProvidersAction::Refresh));
        col.add_child(toolbar.finish());

        if !self.status.is_empty() {
            let tone = provider_status_tone(&self.status);
            col.add_child(status_line(self.status.clone(), self.font, tone));
        }

        col.add_child(
            Container::new(self.byok_form_block())
                .with_margin_top(12.0)
                .with_margin_bottom(12.0)
                .finish(),
        );

        col.add_child(section_title("供应商列表", self.font));
        if self.providers.is_empty() {
            col.add_child(status_line(
                "暂无供应商记录。登录后自动启用 Key Pool；也可上方填写自备 Key。",
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
                let mut provider_col =
                    Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                provider_col.add_child(
                    ui_text::mono(line, self.font)
                        .with_color(theme::text())
                        .finish(),
                );
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
                provider_col.add_child(Container::new(row.finish()).with_margin_top(6.0).finish());
                col.add_child(
                    Container::new(provider_col.finish())
                        .with_uniform_padding(10.0)
                        .with_margin_top(6.0)
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
                        .finish(),
                );
            }
        }

        col.finish()
    }
}

fn provider_status_tone(status: &str) -> StatusTone {
    if status.contains("错误") || status.contains("失败") || status.contains("未找到") {
        StatusTone::Danger
    } else if status.contains("已") || status.contains("成功") || status.contains("就绪") {
        StatusTone::Success
    } else {
        StatusTone::Neutral
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

#[cfg(test)]
mod tests {
    use super::{provider_status_tone, StatusTone};

    #[test]
    fn provider_operation_status_has_explicit_tone() {
        assert_eq!(
            provider_status_tone("刷新供应商失败: timeout"),
            StatusTone::Danger
        );
        assert_eq!(
            provider_status_tone("供应商列表已刷新"),
            StatusTone::Success
        );
        assert_eq!(provider_status_tone("正在查询用量…"), StatusTone::Neutral);
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
            AgentProvidersAction::CycleByokPreset => self.cycle_byok_preset(ctx),
            AgentProvidersAction::CycleByokModel => self.cycle_byok_model(ctx),
            AgentProvidersAction::FocusApiKey => {
                self.api_key_focused = true;
                ctx.notify();
            }
            AgentProvidersAction::ApiKeyEdit(edit) => {
                self.api_key_field.apply(&mut self.api_key_draft, edit);
                ctx.notify();
            }
            AgentProvidersAction::SaveByok => self.save_byok(ctx),
            AgentProvidersAction::TestByok => self.test_byok(ctx),
        }
    }
}
