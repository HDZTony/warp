//! Settings → Cron: Agent scheduled jobs (list / add / run / remove).

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::cron_commands::{
    cron_add, cron_list, cron_remove, cron_run, cron_runs, CronAddParams, CronIdParams, CronJob,
    CronRun, CronRunsParams,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::text_field_input::{
    render_field_with_caret, wrap_text_field_focus_on_click, TextFieldEditAction, TextFieldInput,
    TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum CronAction {
    Refresh,
    Add,
    Run(String),
    Remove(String),
    FocusName,
    FocusExpression,
    FocusPrompt,
    NameEdit(TextFieldEditAction),
    ExpressionEdit(TextFieldEditAction),
    PromptEdit(TextFieldEditAction),
}

pub struct CronView {
    core: CoreHandle,
    font: FamilyId,
    jobs: Vec<CronJob>,
    runs: Vec<CronRun>,
    name_draft: String,
    expression_draft: String,
    prompt_draft: String,
    name_field: TextFieldState,
    expression_field: TextFieldState,
    prompt_field: TextFieldState,
    name_focused: bool,
    expression_focused: bool,
    prompt_focused: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl CronView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            jobs: Vec::new(),
            runs: Vec::new(),
            name_draft: String::new(),
            expression_draft: "0 * * * *".into(),
            prompt_draft: String::new(),
            name_field: TextFieldState::new(),
            expression_field: TextFieldState::new(),
            prompt_field: TextFieldState::new(),
            name_focused: false,
            expression_focused: false,
            prompt_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.cron.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let jobs = cron_list(&state).await?;
                let runs = cron_runs(
                    &state,
                    CronRunsParams {
                        job_id: None,
                        limit: Some(8),
                    },
                )
                .await?;
                Ok::<_, String>((jobs, runs))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((jobs, runs)) => {
                        view.jobs = jobs;
                        view.runs = runs;
                        view.message = wormhole_i18n::t("settings.cron.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message =
                            format!("{}: {error}", wormhole_i18n::t("settings.cron.load_failed"));
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn add_job(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let expression = self.expression_draft.trim().to_string();
        let prompt = self.prompt_draft.trim().to_string();
        if expression.is_empty() || prompt.is_empty() {
            self.message = wormhole_i18n::t("settings.cron.add_need_fields");
            self.tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.cron.adding");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let name = {
            let t = self.name_draft.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        let params = CronAddParams {
            expression,
            prompt,
            name,
            enabled: true,
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cron_add(&state, params).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(_) => {
                        view.prompt_draft.clear();
                        view.name_draft.clear();
                        view.message = wormhole_i18n::t("settings.cron.added");
                        view.tone = StatusTone::Success;
                        view.refresh(ctx);
                    }
                    Err(error) => {
                        view.message =
                            format!("{}: {error}", wormhole_i18n::t("settings.cron.add_failed"));
                        view.tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn run_job(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.cron.running");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cron_run(&state, CronIdParams { id }).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(run) => {
                        view.message = if run.ok {
                            wormhole_i18n::t("settings.cron.run_ok")
                        } else {
                            format!(
                                "{}: {}",
                                wormhole_i18n::t("settings.cron.run_failed"),
                                truncate(&run.output, 120)
                            )
                        };
                        view.tone = if run.ok {
                            StatusTone::Success
                        } else {
                            StatusTone::Danger
                        };
                        view.refresh(ctx);
                    }
                    Err(error) => {
                        view.message =
                            format!("{}: {error}", wormhole_i18n::t("settings.cron.run_failed"));
                        view.tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn remove_job(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.cron.removing");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                cron_remove(&state, CronIdParams { id }).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(_) => {
                        view.message = wormhole_i18n::t("settings.cron.removed");
                        view.tone = StatusTone::Success;
                        view.refresh(ctx);
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.cron.remove_failed")
                        );
                        view.tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn action_chip(
        &self,
        label: &str,
        action: CronAction,
        primary: bool,
        automation_id: &str,
        disabled: bool,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if disabled {
                        theme::placeholder()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if !disabled {
                    ctx.dispatch_typed_action(action.clone());
                }
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

    fn text_field(
        &self,
        label: &str,
        draft: &str,
        field: &TextFieldState,
        focused: bool,
        focus_action: CronAction,
        edit_to_action: fn(TextFieldEditAction) -> CronAction,
        automation_id: &str,
        placeholder: &str,
    ) -> Box<dyn Element> {
        let rendered = render_field_with_caret(
            draft,
            &field.marked_text,
            placeholder,
            self.font,
            focused,
            false,
            false,
            field.cursor,
        );
        let input = TextFieldInput::builder(rendered, move |ctx, action| {
            ctx.dispatch_typed_action(edit_to_action(action));
        })
        .focused(focused)
        .ime_preedit(!field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, move |ctx| {
            ctx.dispatch_typed_action(focus_action.clone());
        });
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                ui_text::body(label.to_string(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(
                EventHandler::new(
                    Container::new(input)
                        .with_uniform_padding(10.0)
                        .with_margin_top(6.0)
                        .with_background(theme::canvas())
                        .with_border(Border::all(1.0).with_border_fill(if focused {
                            theme::accent_cool()
                        } else {
                            theme::border()
                        }))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
                        .finish(),
                )
                .with_automation_id(automation_id)
                .with_automation_label(label.to_string())
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            )
            .finish()
    }

    fn job_row(&self, job: &CronJob) -> Box<dyn Element> {
        let title = job
            .name
            .clone()
            .unwrap_or_else(|| truncate(&job.id, 8));
        let next = job
            .next_run
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| wormhole_i18n::t("settings.cron.no_next"));
        let enabled = if job.enabled {
            wormhole_i18n::t("settings.cron.enabled")
        } else {
            wormhole_i18n::t("settings.cron.disabled")
        };
        let summary = format!(
            "{} · {} · {} · {}",
            title, job.expression, enabled, next
        );
        let id = job.id.clone();
        let id_run = job.id.clone();
        let id_rm = job.id.clone();
        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        row.add_child(
            Container::new(
                ui_text::body(summary, self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_padding_right(12.0)
            .finish(),
        );
        row.add_child(self.action_chip(
            &wormhole_i18n::t("settings.cron.run_now"),
            CronAction::Run(id_run),
            true,
            &format!("settings:cron_run_{id}"),
            self.busy,
        ));
        row.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.cron.remove"),
                CronAction::Remove(id_rm),
                false,
                &format!("settings:cron_remove_{id}"),
                self.busy,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        Container::new(row.finish())
            .with_uniform_padding(10.0)
            .with_margin_top(8.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .finish()
    }

    fn runs_block(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.cron.runs_title"),
            self.font,
        ));
        if self.runs.is_empty() {
            col.add_child(
                Container::new(
                    ui_text::body(wormhole_i18n::t("settings.cron.runs_empty"), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for run in &self.runs {
                let line = format!(
                    "{} · {} · {}",
                    run.started_at.format("%m-%d %H:%M"),
                    if run.ok { "ok" } else { "fail" },
                    truncate(&run.output, 80)
                );
                col.add_child(
                    Container::new(
                        ui_text::body(line, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(6.0)
                    .finish(),
                );
            }
        }
        section_card(col.finish())
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

impl Entity for CronView {
    type Event = ();
}

impl TypedActionView for CronView {
    type Action = CronAction;

    fn handle_action(&mut self, action: &CronAction, ctx: &mut ViewContext<Self>) {
        match action {
            CronAction::Refresh => self.refresh(ctx),
            CronAction::Add => self.add_job(ctx),
            CronAction::Run(id) => self.run_job(id.clone(), ctx),
            CronAction::Remove(id) => self.remove_job(id.clone(), ctx),
            CronAction::FocusName => {
                self.name_focused = true;
                self.expression_focused = false;
                self.prompt_focused = false;
                ctx.notify();
            }
            CronAction::FocusExpression => {
                self.expression_focused = true;
                self.name_focused = false;
                self.prompt_focused = false;
                ctx.notify();
            }
            CronAction::FocusPrompt => {
                self.prompt_focused = true;
                self.name_focused = false;
                self.expression_focused = false;
                ctx.notify();
            }
            CronAction::NameEdit(edit) => {
                self.name_field.apply(&mut self.name_draft, edit);
                ctx.notify();
            }
            CronAction::ExpressionEdit(edit) => {
                self.expression_field
                    .apply(&mut self.expression_draft, edit);
                ctx.notify();
            }
            CronAction::PromptEdit(edit) => {
                self.prompt_field.apply(&mut self.prompt_draft, edit);
                ctx.notify();
            }
        }
    }
}

impl View for CronView {
    fn ui_name() -> &'static str {
        "CronView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.cron.title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.cron.hint"),
            self.font,
        ));
        col.add_child(
            EventHandler::new(status_line(self.message.clone(), self.font, self.tone))
                .with_automation_id("settings:cron_status")
                .with_automation_label(wormhole_i18n::t("settings.cron.status_label"))
                .finish(),
        );

        let mut add_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        add_col.add_child(section_title(
            wormhole_i18n::t("settings.cron.add_title"),
            self.font,
        ));
        add_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.cron.name"),
                &self.name_draft,
                &self.name_field,
                self.name_focused,
                CronAction::FocusName,
                CronAction::NameEdit,
                "settings:cron_name",
                &wormhole_i18n::t("settings.cron.name_placeholder"),
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        add_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.cron.expression"),
                &self.expression_draft,
                &self.expression_field,
                self.expression_focused,
                CronAction::FocusExpression,
                CronAction::ExpressionEdit,
                "settings:cron_expression",
                &wormhole_i18n::t("settings.cron.expression_placeholder"),
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        add_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.cron.prompt"),
                &self.prompt_draft,
                &self.prompt_field,
                self.prompt_focused,
                CronAction::FocusPrompt,
                CronAction::PromptEdit,
                "settings:cron_prompt",
                &wormhole_i18n::t("settings.cron.prompt_placeholder"),
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        let mut add_actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        add_actions.add_child(self.action_chip(
            &wormhole_i18n::t("settings.cron.add"),
            CronAction::Add,
            true,
            "settings:cron_add",
            self.busy,
        ));
        add_col.add_child(
            Container::new(add_actions.finish())
                .with_margin_top(12.0)
                .finish(),
        );
        col.add_child(
            Container::new(section_card(add_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut list_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        list_col.add_child(section_title(
            wormhole_i18n::t("settings.cron.jobs_title"),
            self.font,
        ));
        if self.jobs.is_empty() {
            list_col.add_child(
                Container::new(
                    ui_text::body(wormhole_i18n::t("settings.cron.jobs_empty"), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for job in &self.jobs {
                list_col.add_child(self.job_row(job));
            }
        }
        col.add_child(
            Container::new(section_card(list_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        col.add_child(
            Container::new(self.runs_block())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.action_chip(
            &wormhole_i18n::t("settings.cron.refresh"),
            CronAction::Refresh,
            false,
            "settings:cron_refresh",
            self.busy,
        ));
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        Container::new(col.finish()).finish()
    }
}
