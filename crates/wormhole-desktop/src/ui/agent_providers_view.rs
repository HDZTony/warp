use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_codex_presets::{self, CodexProviderPreset};
use wormhole_desktop_core::agent_llm_commands::{
    self, ConfigureAgentLlmParams, TestAgentLlmConnectionParams,
};
use wormhole_desktop_core::agent_model_routing::{
    self, AgentModelRoutesDto, UpdateAgentModelRoutesParams,
};
use wormhole_desktop_core::agent_ollama::{
    self, local_ai_status_label, ConfigureOllamaParams, EnsureOllamaParams, OllamaStatusDto,
};
use wormhole_desktop_core::agent_provider_commands::{self, AgentModelChoiceDto};
use wormhole_desktop_core::agent_provider_store::AgentProviderSummaryDto;
use wormhole_model_routing::{WorkloadHint, WorkloadRouteTable};

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
    CycleProvider,
    CycleModel,
    FocusApiKey,
    FocusBaseUrl,
    ApiKeyEdit(TextFieldEditAction),
    BaseUrlEdit(TextFieldEditAction),
    SaveProvider,
    TestProvider,
    /// Expand/collapse the model list for one workload row.
    ToggleRoutePicker(usize),
    /// Pick a model for one workload row (`0` = active provider default).
    SelectRouteModel { hint_index: usize, pick: usize },
    SaveRoutes,
    EnsureOllama,
    ToggleOllamaEnabled,
    ApplyOllamaRecommendation,
}

pub struct AgentProvidersView {
    core: CoreHandle,
    font: FamilyId,
    providers: Vec<AgentProviderSummaryDto>,
    active_provider_id: Option<String>,
    llm_summary: String,
    status: String,
    busy: bool,
    /// All builtin presets including `control_plane`.
    presets: Vec<CodexProviderPreset>,
    preset_index: usize,
    model_index: usize,
    api_key_draft: String,
    api_key_field: TextFieldState,
    api_key_focused: bool,
    base_url_draft: String,
    base_url_field: TextFieldState,
    base_url_focused: bool,
    /// Workload route matrix (Automatic Model Routing).
    route_table: WorkloadRouteTable,
    /// Models available in the per-hint picker (`list_agent_model_choices`).
    model_choices: Vec<AgentModelChoiceDto>,
    /// Parallel to [`WorkloadHint::matrix_hints`]: `0` = active provider model, `1+` = `model_choices[i-1]`.
    route_pick_idx: Vec<usize>,
    /// Which workload row has its model list expanded (`None` = all collapsed).
    route_picker_open: Option<usize>,
    ollama: Option<OllamaStatusDto>,
}

fn route_hint_count() -> usize {
    WorkloadHint::matrix_hints().len()
}

fn empty_route_picks() -> Vec<usize> {
    vec![0usize; route_hint_count()]
}

/// Short Chinese title for a workload hint (Settings row label).
fn workload_hint_title(hint: WorkloadHint) -> &'static str {
    match hint {
        WorkloadHint::Chat => "轻量对话",
        WorkloadHint::Reasoning => "规划 / 反思",
        WorkloadHint::Agentic => "主 Agent",
        WorkloadHint::Burst => "高吞吐预扫",
        WorkloadHint::Coding => "代码任务",
        WorkloadHint::Summarization => "Memory 摘要",
        WorkloadHint::Vision => "看图 / OCR",
        WorkloadHint::Subconscious => "潜意识",
    }
}

fn byok_presets() -> Vec<CodexProviderPreset> {
    agent_codex_presets::list_presets()
        .into_iter()
        .filter(|p| p.id != "control_plane")
        .collect()
}

fn is_control_plane_id(id: &str) -> bool {
    id == "control_plane"
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
            llm_summary: "正在加载供应商…".into(),
            status: String::new(),
            busy: false,
            presets,
            preset_index: 0,
            model_index: 0,
            api_key_draft: String::new(),
            api_key_field: TextFieldState::new(),
            api_key_focused: false,
            base_url_draft: String::new(),
            base_url_field: TextFieldState::new(),
            base_url_focused: false,
            route_table: WorkloadRouteTable::default(),
            model_choices: Vec::new(),
            route_pick_idx: empty_route_picks(),
            route_picker_open: None,
            ollama: None,
        };
        if let Some(preset) = view.current_preset() {
            view.base_url_draft = preset.default_base_url.clone();
        }
        view.refresh(ctx);
        view
    }

    fn current_preset(&self) -> Option<&CodexProviderPreset> {
        self.presets.get(self.preset_index)
    }

    fn current_model_id(&self) -> String {
        let Some(preset) = self.current_preset() else {
            return String::new();
        };
        preset
            .models
            .get(self.model_index)
            .map(|m| m.id.clone())
            .unwrap_or_else(|| preset.default_model.clone())
    }

    fn restore_drafts_for_preset(&mut self) {
        let Some(preset) = self.current_preset().cloned() else {
            return;
        };
        self.model_index = 0;
        self.api_key_draft.clear();
        self.api_key_field = TextFieldState::new();
        self.api_key_focused = false;
        if let Some(existing) = self.providers.iter().find(|p| p.id == preset.id) {
            self.base_url_draft = if existing.base_url.trim().is_empty() {
                preset.default_base_url.clone()
            } else {
                existing.base_url.clone()
            };
            if let Some(idx) = preset.models.iter().position(|m| m.id == existing.model) {
                self.model_index = idx;
            }
        } else {
            self.base_url_draft = preset.default_base_url.clone();
        }
        self.base_url_field = TextFieldState::new();
        self.base_url_focused = false;
    }

    fn cycle_provider(&mut self, ctx: &mut ViewContext<Self>) {
        if self.presets.is_empty() {
            return;
        }
        self.preset_index = (self.preset_index + 1) % self.presets.len();
        self.restore_drafts_for_preset();
        if let Some(preset) = self.current_preset() {
            self.status = format!("已切换到 {}", preset.name);
        }
        ctx.notify();
    }

    fn cycle_model(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_preset() else {
            return;
        };
        if preset.models.is_empty() {
            return;
        }
        self.model_index = (self.model_index + 1) % preset.models.len();
        ctx.notify();
    }

    fn save_provider(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_preset().cloned() else {
            self.status = "没有可用的供应商预设".into();
            ctx.notify();
            return;
        };
        let api_key = self.api_key_draft.trim().to_string();
        if api_key.is_empty() {
            // Allow saving model/base_url updates when key already configured.
            let has_existing = self
                .providers
                .iter()
                .any(|p| p.id == preset.id && p.api_key_configured);
            if !has_existing {
                self.status = "请先填写当前供应商的 API Key".into();
                ctx.notify();
                return;
            }
        }
        let model = self.current_model_id();
        let base_url = {
            let draft = self.base_url_draft.trim();
            if draft.is_empty() {
                preset.default_base_url.clone()
            } else {
                draft.to_string()
            }
        };
        self.busy = true;
        self.status = format!("正在保存 {}…", preset.name);
        ctx.notify();
        let core = self.core.clone();
        let api_key_param = if api_key.is_empty() {
            None
        } else {
            Some(api_key)
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_llm_commands::configure_agent_llm(
                    &state,
                    ConfigureAgentLlmParams {
                        provider: Some(preset.id),
                        api_key: api_key_param,
                        model: Some(model),
                        base_url: Some(base_url),
                    },
                )
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(cfg) => {
                        view.status =
                            format!("配置已安全保存 · {} · {}", cfg.provider, cfg.model);
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

    fn test_provider(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(preset) = self.current_preset().cloned() else {
            self.status = "没有可用的供应商预设".into();
            ctx.notify();
            return;
        };
        let api_key = self.api_key_draft.trim().to_string();
        let base_url = {
            let draft = self.base_url_draft.trim();
            if draft.is_empty() {
                preset.default_base_url.clone()
            } else {
                draft.to_string()
            }
        };
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
                        base_url: Some(base_url),
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

    fn api_key_field_el(&self) -> Box<dyn Element> {
        let field = render_field_with_caret(
            &self.api_key_draft,
            &self.api_key_field.marked_text,
            "输入当前供应商的 API Key",
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
            .with_vertical_margin(4.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(if self.api_key_focused {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
            .finish()
    }

    fn base_url_field_el(&self) -> Box<dyn Element> {
        let field = render_field_with_caret(
            &self.base_url_draft,
            &self.base_url_field.marked_text,
            "https://api.example.com/v1",
            self.font,
            self.base_url_focused,
            false,
            false,
            self.base_url_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(AgentProvidersAction::BaseUrlEdit(action));
        })
        .focused(self.base_url_focused)
        .ime_preedit(!self.base_url_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(AgentProvidersAction::FocusBaseUrl);
        });
        Container::new(input)
            .with_uniform_padding(10.0)
            .with_vertical_margin(4.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(if self.base_url_focused {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
            .finish()
    }

    fn form_field_shell(&self, label: &str, value: Box<dyn Element>) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(status_line(label.to_string(), self.font, StatusTone::Muted));
        col.add_child(Container::new(value).with_margin_top(6.0).finish());
        Container::new(col.finish())
            .with_uniform_padding(2.0)
            .finish()
    }

    fn selectable_value_el(
        &self,
        value: &str,
        cycle_label: &str,
        cycle_automation_id: &str,
        cycle: AgentProvidersAction,
    ) -> Box<dyn Element> {
        let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        // Expanded keeps the value box finite inside Stretch columns / row grids.
        row.add_child(Expanded::new(
            1.0,
            Container::new(
                ui_text::body(value.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_uniform_padding(10.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
            .finish(),
        )
        .finish());
        row.add_child(
            Container::new(self.action_button_with_id(cycle_label, cycle_automation_id, cycle))
                .with_margin_left(8.0)
                .finish(),
        );
        row.finish()
    }

    fn provider_form_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("AGENT · CODEX 供应商", self.font));
        col.add_child(section_hint(
            "参考 OpenCode 的 provider 配置：每个供应商独立保存 API Key、Base URL 和模型，切换后立即应用。密钥仅保存在本机并直连厂商。",
            self.font,
        ));

        let preset = self.current_preset();
        let provider_label = preset
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "—".into());
        let model_label = preset
            .and_then(|p| p.models.get(self.model_index))
            .map(|m| m.label.clone())
            .unwrap_or_else(|| self.current_model_id());
        let base_url_display = if self.base_url_draft.trim().is_empty() {
            preset
                .map(|p| p.default_base_url.as_str())
                .unwrap_or("—")
        } else {
            self.base_url_draft.trim()
        };
        let detail = format!("当前：{provider_label} · 模型 {model_label} · {base_url_display}");

        // Row 1: 供应商 | 模型（2×2 上半）— Expanded 避免 row 子项拿到无限宽。
        let mut row1 = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Start);
        row1.add_child(
            Expanded::new(
                1.0,
                self.form_field_shell(
                    "供应商",
                    self.selectable_value_el(
                        &provider_label,
                        "下一项",
                        "settings:agent_provider:cycle",
                        AgentProvidersAction::CycleProvider,
                    ),
                ),
            )
            .finish(),
        );
        row1.add_child(
            Expanded::new(
                1.0,
                Container::new(self.form_field_shell(
                    "模型",
                    self.selectable_value_el(
                        &model_label,
                        "下一项",
                        "settings:agent_model:cycle",
                        AgentProvidersAction::CycleModel,
                    ),
                ))
                .with_margin_left(10.0)
                .finish(),
            )
            .finish(),
        );
        col.add_child(Container::new(row1.finish()).with_margin_top(10.0).finish());

        // Row 2: Base URL | API Key（2×2 下半）
        let mut row2 = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Start);
        row2.add_child(
            Expanded::new(
                1.0,
                self.form_field_shell("Base URL", self.base_url_field_el()),
            )
            .finish(),
        );
        row2.add_child(
            Expanded::new(
                1.0,
                Container::new(self.form_field_shell("API Key", self.api_key_field_el()))
                    .with_margin_left(10.0)
                    .finish(),
            )
            .finish(),
        );
        col.add_child(Container::new(row2.finish()).with_margin_top(10.0).finish());

        let mut action_row = Flex::row();
        action_row.add_child(self.action_button("保存配置", AgentProvidersAction::SaveProvider));
        action_row.add_child(
            Container::new(self.action_button("测试连接", AgentProvidersAction::TestProvider))
                .with_margin_left(8.0)
                .finish(),
        );
        action_row.add_child(
            Container::new(self.action_button("刷新", AgentProvidersAction::Refresh))
                .with_margin_left(8.0)
                .finish(),
        );
        col.add_child(
            Container::new(action_row.finish())
                .with_margin_top(10.0)
                .finish(),
        );
        col.add_child(status_line(detail, self.font, StatusTone::Muted));
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
                let routes = agent_model_routing::get_agent_model_routes(&state).await;
                let ollama = agent_ollama::ollama_status(&state).await;
                let choices = agent_provider_commands::list_agent_model_choices(&state).await;
                (providers, llm, routes, ollama, choices)
            },
            |view, output, ctx| {
                let (providers, llm, routes, ollama, choices) = output;
                match providers {
                    Ok(list) => {
                        view.active_provider_id = list.active_provider_id.clone();
                        view.providers = list.providers;
                        if let Some(active) = view.active_provider_id.clone() {
                            // Prefer syncing the form to the active BYOK provider; platform
                            // path is not listed in this settings UI.
                            if !is_control_plane_id(&active) {
                                if let Some(idx) = view.presets.iter().position(|p| p.id == active)
                                {
                                    view.preset_index = idx;
                                    view.restore_drafts_for_preset();
                                }
                            }
                        }
                        view.status = "供应商列表已刷新".into();
                    }
                    Err(error) => {
                        view.status = format!("刷新失败：{error}");
                    }
                }
                match choices {
                    Ok(dto) => view.model_choices = dto.choices,
                    Err(err) => {
                        view.status = format!("模型列表读取失败：{err}");
                    }
                }
                if let Ok(routes) = routes {
                    view.apply_routes_dto(routes);
                }
                match ollama {
                    Ok(status) => view.ollama = Some(status),
                    Err(err) => {
                        view.status = format!("Ollama 状态读取失败：{err}");
                    }
                }
                view.llm_summary = match llm {
                    Ok(cfg) if is_control_plane_id(&cfg.provider) => {
                        format!("当前：平台模型 · {}", cfg.model)
                    }
                    Ok(cfg) => {
                        format!(
                            "当前：{} · {} · {} · BYOK 直连",
                            cfg.provider, cfg.model, cfg.base_url
                        )
                    }
                    Err(err) => format!("LLM 配置错误：{err}"),
                };
                view.busy = false;
                ctx.notify();
            },
        );
    }

    fn apply_routes_dto(&mut self, dto: AgentModelRoutesDto) {
        self.route_table = dto.routes;
        self.sync_route_picks_from_table();
    }

    fn ensure_orphan_choice(&mut self, provider_id: Option<&str>, model: &str) -> usize {
        let model = model.trim();
        if let Some(idx) = self.find_choice_index(provider_id, model) {
            return idx + 1;
        }
        let provider = provider_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("unknown");
        let label = if provider == "unknown" {
            format!("{model}（已保存）")
        } else {
            format!("{provider} · {model}（已保存）")
        };
        self.model_choices.push(AgentModelChoiceDto {
            id: format!("{provider}:{model}"),
            provider_id: provider.to_string(),
            model: model.to_string(),
            label,
            source: "saved".into(),
            is_active: false,
        });
        self.model_choices.len()
    }

    fn find_choice_index(&self, provider_id: Option<&str>, model: &str) -> Option<usize> {
        let model = model.trim();
        if model.is_empty() {
            return None;
        }
        let provider = provider_id.map(str::trim).filter(|s| !s.is_empty());
        if let Some(p) = provider {
            return self
                .model_choices
                .iter()
                .position(|c| c.provider_id == p && c.model == model);
        }
        // Model-only binding: only auto-match when exactly one provider owns that model.
        let matches: Vec<usize> = self
            .model_choices
            .iter()
            .enumerate()
            .filter(|(_, c)| c.model == model)
            .map(|(i, _)| i)
            .collect();
        if matches.len() == 1 {
            Some(matches[0])
        } else {
            None
        }
    }

    fn sync_route_picks_from_table(&mut self) {
        let hints = WorkloadHint::matrix_hints();
        if self.route_pick_idx.len() != hints.len() {
            self.route_pick_idx = empty_route_picks();
        }
        let snapshot: Vec<(Option<String>, Option<String>)> = hints
            .iter()
            .map(|h| {
                let binding = self.route_table.binding_for(*h);
                (
                    binding.provider_id.clone(),
                    binding
                        .model
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                )
            })
            .collect();
        for (i, (provider_id, model)) in snapshot.into_iter().enumerate() {
            self.route_pick_idx[i] = match model.as_deref() {
                None => 0,
                Some(m) => self.ensure_orphan_choice(provider_id.as_deref(), m),
            };
        }
    }

    /// Concrete label for the active Agent provider/model (used when a route has no override).
    fn active_model_label(&self) -> String {
        if let Some(choice) = self.model_choices.iter().find(|c| c.is_active) {
            return choice.label.clone();
        }
        if let Some(active_id) = self.active_provider_id.as_deref() {
            if let Some(choice) = self
                .model_choices
                .iter()
                .find(|c| c.provider_id == active_id)
            {
                return choice.label.clone();
            }
            if let Some(provider) = self.providers.iter().find(|p| p.id == active_id) {
                let name = provider.name.trim();
                let model = provider.model.trim();
                if !name.is_empty() && !model.is_empty() {
                    return format!("{name} · {model}");
                }
                if !model.is_empty() {
                    return model.to_string();
                }
            }
        }
        "未配置模型".into()
    }

    fn route_pick_label(&self, index: usize) -> String {
        let pick = self.route_pick_idx.get(index).copied().unwrap_or(0);
        if pick == 0 {
            return self.active_model_label();
        }
        self.model_choices
            .get(pick - 1)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| self.active_model_label())
    }

    fn apply_pick_to_binding(&mut self, index: usize) {
        let hints = WorkloadHint::matrix_hints();
        let Some(hint) = hints.get(index).copied() else {
            return;
        };
        let pick = self.route_pick_idx.get(index).copied().unwrap_or(0);
        let binding = self.route_table.binding_for_mut(hint);
        if pick == 0 {
            binding.provider_id = None;
            binding.model = None;
            return;
        }
        let Some(choice) = self.model_choices.get(pick - 1).cloned() else {
            binding.provider_id = None;
            binding.model = None;
            return;
        };
        binding.provider_id = Some(choice.provider_id);
        binding.model = Some(choice.model);
    }

    fn save_routes(&mut self, ctx: &mut ViewContext<Self>) {
        for i in 0..route_hint_count() {
            self.apply_pick_to_binding(i);
        }
        self.busy = true;
        self.status = "正在保存 Workload Routes…".into();
        ctx.notify();
        let core = self.core.clone();
        let params = UpdateAgentModelRoutesParams {
            routes: self.route_table.clone(),
            orchestrator_model: None,
            teams: Default::default(),
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_model_routing::update_agent_model_routes(&state, params).await
            },
            |view, result, ctx| {
                match result {
                    Ok(dto) => {
                        view.apply_routes_dto(dto);
                        view.status = "Workload Routes 已保存".into();
                    }
                    Err(err) => view.status = format!("保存 Routes 失败：{err}"),
                }
                view.busy = false;
                ctx.notify();
            },
        );
    }

    fn toggle_route_picker(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if index >= route_hint_count() {
            return;
        }
        self.route_picker_open = if self.route_picker_open == Some(index) {
            None
        } else {
            Some(index)
        };
        ctx.notify();
    }

    fn select_route_model(&mut self, hint_index: usize, pick: usize, ctx: &mut ViewContext<Self>) {
        if hint_index >= route_hint_count() {
            return;
        }
        let max = self.model_choices.len();
        if pick > max {
            return;
        }
        if let Some(slot) = self.route_pick_idx.get_mut(hint_index) {
            *slot = pick;
        }
        self.route_picker_open = None;
        let title = WorkloadHint::matrix_hints()
            .get(hint_index)
            .copied()
            .map(workload_hint_title)
            .unwrap_or("?");
        self.status = format!("{title} → {}", self.route_pick_label(hint_index));
        ctx.notify();
    }

    fn route_option_label(&self, pick: usize) -> String {
        if pick == 0 {
            format!("当前供应商 · {}", self.active_model_label())
        } else {
            self.model_choices
                .get(pick - 1)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| self.active_model_label())
        }
    }

    fn route_picker_list(&self, hint_index: usize, hint: WorkloadHint) -> Box<dyn Element> {
        let selected = self.route_pick_idx.get(hint_index).copied().unwrap_or(0);
        let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let option_count = self.model_choices.len() + 1;
        for pick in 0..option_count {
            let label = self.route_option_label(pick);
            let mark = if pick == selected { "✓ " } else { "  " };
            let btn = self.action_button_with_id(
                &format!("{mark}{label}"),
                &format!(
                    "settings:agent_routes:option:{}:{}",
                    hint.as_str(),
                    pick
                ),
                AgentProvidersAction::SelectRouteModel {
                    hint_index,
                    pick,
                },
            );
            list.add_child(
                Container::new(btn)
                    .with_margin_top(if pick == 0 { 0.0 } else { 4.0 })
                    .finish(),
            );
        }
        Container::new(list.finish())
            .with_margin_top(6.0)
            .with_margin_left(108.0)
            .with_uniform_padding(8.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
            .finish()
    }

    fn routes_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("Workload Routes", self.font));
        col.add_child(section_hint(
            "按任务选模型。点「选择」展开全部可选模型（可同时看到 Ollama / DeepSeek 等）。每行独立；未覆盖时用上方当前供应商。改完点「保存全部」。",
            self.font,
        ));
        for (i, h) in WorkloadHint::matrix_hints().iter().enumerate() {
            let title = workload_hint_title(*h);
            let pick_label = self.route_pick_label(i);
            let open = self.route_picker_open == Some(i);
            let toggle_label = if open { "收起" } else { "选择" };
            let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
            row.add_child(
                ConstrainedBox::new(
                    ui_text::body(title.to_string(), self.font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_width(108.0)
                .finish(),
            );
            row.add_child(
                Expanded::new(
                    1.0,
                    self.selectable_value_el(
                        &pick_label,
                        toggle_label,
                        &format!("settings:agent_routes:pick:{}", h.as_str()),
                        AgentProvidersAction::ToggleRoutePicker(i),
                    ),
                )
                .finish(),
            );
            col.add_child(
                Container::new(row.finish())
                    .with_margin_top(6.0)
                    .finish(),
            );
            if open {
                col.add_child(self.route_picker_list(i, *h));
            }
        }
        col.add_child(
            Container::new(self.action_button_with_id(
                "保存全部 Routes",
                "settings:agent_routes:save",
                AgentProvidersAction::SaveRoutes,
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        col.finish()
    }

    fn local_ai_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title("Local AI (Ollama)", self.font));
        col.add_child(section_hint(
            "启动 → 启用本机 → Memory 用本机 embeddings；chat/summarization 绑 ollama 后走本机。主 Agent（agentic）默认仍用上方当前供应商。",
            self.font,
        ));

        let (status_label, enabled, chat, embed, failed, binary, daemon, in_progress) =
            match self.ollama.as_ref() {
                Some(s) => {
                    let failed = matches!(s.phase, agent_ollama::OllamaPhase::Failed)
                        || s.last_error.is_some();
                    let in_progress = matches!(
                        s.progress.phase.as_str(),
                        "installing" | "starting" | "pulling" | "recommend" | "detect" | "register"
                    ) && !s.progress.message.is_empty();
                    (
                        local_ai_status_label(s.binary_on_path, s.daemon_reachable, failed)
                            .to_string(),
                        s.config.enabled,
                        s.config.chat_model.clone(),
                        s.config.embedding_model.clone(),
                        failed,
                        s.binary_on_path,
                        s.daemon_reachable,
                        if in_progress {
                            Some(format!("{}% {}", s.progress.percent, s.progress.message))
                        } else {
                            None
                        },
                    )
                }
                None => (
                    "未知".into(),
                    false,
                    String::new(),
                    String::new(),
                    false,
                    false,
                    false,
                    None,
                ),
            };

        let status_tone = if failed {
            StatusTone::Danger
        } else if daemon {
            StatusTone::Success
        } else {
            StatusTone::Neutral
        };
        col.add_child(status_line(
            format!(
                "状态：{status_label}{}",
                if enabled { " · 本机已启用" } else { " · 本机未启用" }
            ),
            self.font,
            status_tone,
        ));
        if !chat.is_empty() {
            col.add_child(status_line(
                format!("模型：chat={chat} · embed={embed}"),
                self.font,
                StatusTone::Muted,
            ));
        }
        if let Some(progress) = in_progress {
            col.add_child(status_line(progress, self.font, StatusTone::Neutral));
        }
        if let Some(err) = self.ollama.as_ref().and_then(|s| s.last_error.clone()) {
            if failed {
                col.add_child(status_line(err, self.font, StatusTone::Danger));
            }
        }

        let primary = if !binary {
            "安装并启动"
        } else if !daemon {
            "启动"
        } else {
            "重新检测"
        };

        let mut row = Flex::row();
        row.add_child(self.action_button_with_id(
            primary,
            "settings:agent_local_ai:ensure",
            AgentProvidersAction::EnsureOllama,
        ));
        row.add_child(
            Container::new(self.action_button_with_id(
                if enabled {
                    "关闭本机"
                } else {
                    "启用本机"
                },
                "settings:agent_local_ai:toggle",
                AgentProvidersAction::ToggleOllamaEnabled,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        if !daemon {
            row.add_child(
                Container::new(self.action_button_with_id(
                    "应用推荐",
                    "settings:agent_local_ai:apply_rec",
                    AgentProvidersAction::ApplyOllamaRecommendation,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(row.finish())
                .with_margin_top(8.0)
                .finish(),
        );
        col.finish()
    }

    fn ensure_ollama(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在安装 / 启动 Ollama…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_ollama::ensure_ollama(
                    &state,
                    EnsureOllamaParams {
                        pull_models: None,
                        enable: Some(true),
                        bind_summarization_route: Some(true),
                        apply_recommendation: None,
                    },
                )
                .await
            },
            |view, result, ctx| {
                match result {
                    Ok(status) => {
                        view.ollama = Some(status);
                        view.status = "Ollama 已就绪".into();
                    }
                    Err(err) => view.status = format!("Ollama 失败：{err}"),
                }
                view.busy = false;
                view.refresh(ctx);
            },
        );
    }

    fn toggle_ollama_enabled(&mut self, ctx: &mut ViewContext<Self>) {
        let enabled = self
            .ollama
            .as_ref()
            .map(|s| s.config.enabled)
            .unwrap_or(false);
        self.busy = true;
        self.status = if enabled {
            "正在关闭本机 embeddings…".into()
        } else {
            "正在启用本机 embeddings…".into()
        };
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_ollama::configure_ollama(
                    &state,
                    ConfigureOllamaParams {
                        enabled: Some(!enabled),
                        chat_model: None,
                        embedding_model: None,
                        pull_models_on_ensure: None,
                        bind_summarization_route: None,
                        apply_recommendation: None,
                    },
                )
                .await
            },
            |view, result, ctx| {
                match result {
                    Ok(status) => {
                        let on = status.config.enabled;
                        view.ollama = Some(status);
                        view.status = if on {
                            "本机 embeddings 已启用（需守护进程可达）".into()
                        } else {
                            "本机 embeddings 已关闭".into()
                        };
                    }
                    Err(err) => view.status = format!("配置失败：{err}"),
                }
                view.busy = false;
                ctx.notify();
            },
        );
    }

    fn apply_ollama_recommendation(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.status = "正在应用设备推荐模型…".into();
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                agent_ollama::configure_ollama(
                    &state,
                    ConfigureOllamaParams {
                        enabled: None,
                        chat_model: None,
                        embedding_model: None,
                        pull_models_on_ensure: None,
                        bind_summarization_route: None,
                        apply_recommendation: Some(true),
                    },
                )
                .await
            },
            |view, result, ctx| {
                match result {
                    Ok(status) => {
                        let chat = status.config.chat_model.clone();
                        view.ollama = Some(status);
                        view.status = format!("已套用设备推荐：{chat}");
                    }
                    Err(err) => view.status = format!("应用推荐失败：{err}"),
                }
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
                    Ok(_) => "供应商已启用".into(),
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
                    Ok(_) => "供应商已删除".into(),
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
        self.action_button_with_id(label, &format!("settings:agent_btn:{label}"), action)
    }

    fn action_blocks_when_busy(action: &AgentProvidersAction) -> bool {
        matches!(
            action,
            AgentProvidersAction::Refresh
                | AgentProvidersAction::SaveProvider
                | AgentProvidersAction::TestProvider
                | AgentProvidersAction::SaveRoutes
                | AgentProvidersAction::EnsureOllama
                | AgentProvidersAction::ToggleOllamaEnabled
                | AgentProvidersAction::ApplyOllamaRecommendation
                | AgentProvidersAction::Activate(_)
                | AgentProvidersAction::Delete(_)
                | AgentProvidersAction::QueryUsage(_)
        )
    }

    fn action_button_with_id(
        &self,
        label: &str,
        automation_id: &str,
        action: AgentProvidersAction,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        let automation_id = automation_id.to_string();
        let disabled = self.busy && Self::action_blocks_when_busy(&action);
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if disabled {
                        theme::muted()
                    } else {
                        theme::accent()
                    })
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
        .with_uniform_padding(8.0)
        .with_background(theme::accent_bg(28))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
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
        col.add_child(status_line(
            self.llm_summary.clone(),
            self.font,
            StatusTone::Muted,
        ));

        if !self.status.is_empty() {
            let tone = provider_status_tone(&self.status);
            col.add_child(status_line(self.status.clone(), self.font, tone));
        }

        col.add_child(
            Container::new(self.provider_form_block())
                .with_margin_top(8.0)
                .with_margin_bottom(12.0)
                .finish(),
        );

        col.add_child(section_title("已配置供应商", self.font));
        let byok_providers: Vec<_> = self
            .providers
            .iter()
            .filter(|p| !is_control_plane_id(&p.id))
            .collect();
        if byok_providers.is_empty() {
            col.add_child(status_line(
                "暂无自备供应商。在上方填写 API Key 并保存配置。",
                self.font,
                StatusTone::Placeholder,
            ));
        } else {
            for provider in byok_providers {
                let active = provider.is_active;
                let path = if provider.chat_completions_upstream {
                    " · 本地代理"
                } else {
                    " · Responses 直连"
                };
                let line = truncate_middle(
                    &format!(
                        "{}{} · {} · {}{path}",
                        provider.name,
                        if active { " [当前]" } else { "" },
                        provider.model,
                        provider.base_url,
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
                        self.action_button("用量", AgentProvidersAction::QueryUsage(usage_id)),
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
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                        .finish(),
                );
            }
        }

        col.add_child(
            Container::new(self.local_ai_block())
                .with_margin_top(16.0)
                .finish(),
        );

        col.add_child(
            Container::new(self.routes_block())
                .with_margin_top(16.0)
                .finish(),
        );

        col.finish()
    }
}

fn provider_status_tone(status: &str) -> StatusTone {
    let lower = status.to_ascii_lowercase();
    if lower.contains("fail")
        || lower.contains("error")
        || status.contains("失败")
        || status.contains("错误")
    {
        StatusTone::Danger
    } else if lower.contains("saved")
        || lower.contains("success")
        || lower.contains("refreshed")
        || status.contains("成功")
        || status.contains("已")
    {
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
            .unwrap_or_else(|| "Usage query failed".into());
    }
    let Some(items) = result.data.as_ref().filter(|items| !items.is_empty()) else {
        return "Usage query succeeded (no data)".into();
    };
    let first = &items[0];
    if first.is_valid == Some(false) {
        return first
            .invalid_message
            .clone()
            .unwrap_or_else(|| "Usage invalid".into());
    }
    let mut parts = Vec::new();
    if let Some(plan) = &first.plan_name {
        parts.push(plan.clone());
    }
    if let (Some(remaining), Some(unit)) = (first.remaining, &first.unit) {
        parts.push(format!("remaining {remaining} {unit}"));
    } else if let (Some(used), Some(total)) = (first.used, first.total) {
        parts.push(format!("used {used} / {total}"));
    }
    if let Some(extra) = &first.extra {
        parts.push(extra.clone());
    }
    if parts.is_empty() {
        "Usage query succeeded".into()
    } else {
        parts.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::{is_control_plane_id, provider_status_tone, StatusTone};

    #[test]
    fn provider_operation_status_has_explicit_tone() {
        assert_eq!(
            provider_status_tone("刷新失败：timeout"),
            StatusTone::Danger
        );
        assert_eq!(
            provider_status_tone("供应商列表已刷新"),
            StatusTone::Success
        );
        assert_eq!(provider_status_tone("正在查询用量…"), StatusTone::Neutral);
    }

    #[test]
    fn workload_hint_titles_cover_matrix() {
        for hint in WorkloadHint::matrix_hints() {
            assert!(!workload_hint_title(*hint).is_empty());
        }
        assert_eq!(workload_hint_title(WorkloadHint::Agentic), "主 Agent");
        assert_eq!(workload_hint_title(WorkloadHint::Chat), "轻量对话");
    }

    #[test]
    fn control_plane_is_hidden_from_settings_ui_helpers() {
        assert!(is_control_plane_id("control_plane"));
        assert!(!is_control_plane_id("zai"));
        let byok: Vec<_> = super::byok_presets()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert!(!byok.iter().any(|id| id == "control_plane"));
        assert!(byok.iter().any(|id| id == "zai"));
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
            AgentProvidersAction::CycleProvider => self.cycle_provider(ctx),
            AgentProvidersAction::CycleModel => self.cycle_model(ctx),
            AgentProvidersAction::FocusApiKey => {
                self.api_key_focused = true;
                self.base_url_focused = false;
                ctx.notify();
            }
            AgentProvidersAction::FocusBaseUrl => {
                self.base_url_focused = true;
                self.api_key_focused = false;
                ctx.notify();
            }
            AgentProvidersAction::ApiKeyEdit(edit) => {
                self.api_key_field.apply(&mut self.api_key_draft, edit);
                ctx.notify();
            }
            AgentProvidersAction::BaseUrlEdit(edit) => {
                self.base_url_field.apply(&mut self.base_url_draft, edit);
                ctx.notify();
            }
            AgentProvidersAction::SaveProvider => self.save_provider(ctx),
            AgentProvidersAction::TestProvider => self.test_provider(ctx),
            AgentProvidersAction::ToggleRoutePicker(index) => self.toggle_route_picker(*index, ctx),
            AgentProvidersAction::SelectRouteModel { hint_index, pick } => {
                self.select_route_model(*hint_index, *pick, ctx)
            }
            AgentProvidersAction::SaveRoutes => self.save_routes(ctx),
            AgentProvidersAction::EnsureOllama => self.ensure_ollama(ctx),
            AgentProvidersAction::ToggleOllamaEnabled => self.toggle_ollama_enabled(ctx),
            AgentProvidersAction::ApplyOllamaRecommendation => {
                self.apply_ollama_recommendation(ctx)
            }
        }
    }
}
