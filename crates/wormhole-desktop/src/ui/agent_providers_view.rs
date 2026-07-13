use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_provider_commands;
use wormhole_desktop_core::agent_provider_store::AgentProviderSummaryDto;

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
}

pub struct AgentProvidersView {
    core: CoreHandle,
    font: FamilyId,
    providers: Vec<AgentProviderSummaryDto>,
    active_provider_id: Option<String>,
    llm_summary: String,
    status: String,
}

impl AgentProvidersView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            providers: Vec::new(),
            active_provider_id: None,
            llm_summary: "加载 Codex 上游…".into(),
            status: String::new(),
        };
        view.refresh(ctx);
        view
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
        col.add_child(section_hint(
            "登录后使用 control plane Key Pool（control_plane）。不拦截 ccswitch://；CC Switch 桌面程序可独立使用。",
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

        col.add_child(section_title("供应商列表", self.font));
        if self.providers.is_empty() {
            col.add_child(status_line(
                "暂无供应商记录。登录后将自动启用远端 Key Pool。",
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
        }
    }
}
