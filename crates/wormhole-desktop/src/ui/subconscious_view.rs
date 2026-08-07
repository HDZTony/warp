//! Settings → Subconscious: background memory-world reflection loop.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::agent_subconscious::{
    subconscious_settings_get, subconscious_settings_set, subconscious_status, subconscious_trigger,
    HeartbeatConfig, SubconsciousSettingsUpdateParams, SubconsciousStatus,
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
struct Draft {
    mode: String,
    inference_enabled: bool,
    interval_minutes: u64,
}

impl Draft {
    fn from_cfg(cfg: &HeartbeatConfig) -> Self {
        Self {
            mode: cfg.effective_mode().as_str().to_string(),
            inference_enabled: cfg.inference_enabled,
            interval_minutes: cfg.clamped_interval_minutes(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SubconsciousAction {
    Refresh,
    Save,
    RunNow,
    SetMode(&'static str),
    ToggleInference,
    FocusInterval,
    IntervalEdit(TextFieldEditAction),
}

pub struct SubconsciousView {
    core: CoreHandle,
    font: FamilyId,
    draft: Option<Draft>,
    status: Option<SubconsciousStatus>,
    interval_draft: String,
    interval_field: TextFieldState,
    interval_focused: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

fn mode_label(mode: &str) -> String {
    match mode {
        "simple" => wormhole_i18n::t("settings.subconscious.mode_simple"),
        "aggressive" => wormhole_i18n::t("settings.subconscious.mode_aggressive"),
        _ => wormhole_i18n::t("settings.subconscious.mode_off"),
    }
}

fn mode_hint(mode: &str) -> String {
    match mode {
        "simple" => wormhole_i18n::t("settings.subconscious.mode_simple_hint"),
        "aggressive" => wormhole_i18n::t("settings.subconscious.mode_aggressive_hint"),
        _ => wormhole_i18n::t("settings.subconscious.mode_off_hint"),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn format_relative_ago(last_ms: Option<u64>) -> String {
    let Some(last) = last_ms else {
        return wormhole_i18n::t("settings.subconscious.never");
    };
    let ago_ms = now_ms().saturating_sub(last);
    let secs = ago_ms / 1000;
    if secs < 60 {
        return wormhole_i18n::t("settings.subconscious.ago_just_now");
    }
    let mins = secs / 60;
    if mins < 60 {
        return wormhole_i18n::t_args(
            "settings.subconscious.ago_minutes",
            &[("n", &mins.to_string())],
        );
    }
    let hours = mins / 60;
    if hours < 48 {
        return wormhole_i18n::t_args(
            "settings.subconscious.ago_hours",
            &[("n", &hours.to_string())],
        );
    }
    let days = hours / 24;
    wormhole_i18n::t_args("settings.subconscious.ago_days", &[("n", &days.to_string())])
}

impl SubconsciousView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            draft: None,
            status: None,
            interval_draft: String::new(),
            interval_field: TextFieldState::new(),
            interval_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn apply_draft(&mut self, draft: Draft) {
        self.interval_draft = draft.interval_minutes.to_string();
        self.interval_field = TextFieldState::new();
        self.interval_field.clamp_cursor(&self.interval_draft);
        self.draft = Some(draft);
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.subconscious.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let settings = subconscious_settings_get(&state).await?;
                let status = subconscious_status(&state).await?;
                Ok::<_, String>((settings, status))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((settings, status)) => {
                        view.apply_draft(Draft::from_cfg(&settings));
                        view.status = Some(status);
                        view.message = wormhole_i18n::t("settings.subconscious.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.subconscious.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn save(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        if let Ok(parsed) = self.interval_draft.trim().parse::<u64>() {
            draft.interval_minutes = parsed;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.subconscious.saving");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let params = SubconsciousSettingsUpdateParams {
            mode: Some(draft.mode.clone()),
            enabled: None,
            interval_minutes: Some(draft.interval_minutes),
            inference_enabled: Some(draft.inference_enabled),
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let cfg = subconscious_settings_set(&state, params).await?;
                let status = subconscious_status(&state).await?;
                Ok::<_, String>((cfg, status))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((cfg, status)) => {
                        view.apply_draft(Draft::from_cfg(&cfg));
                        view.status = Some(status);
                        view.message = wormhole_i18n::t("settings.subconscious.saved");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.subconscious.save_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn run_now(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.subconscious.running");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let trigger = subconscious_trigger(&state).await?;
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                let status = subconscious_status(&state).await?;
                Ok::<_, String>((trigger, status))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((trigger, status)) => {
                        view.status = Some(status);
                        if trigger.accepted {
                            view.message = wormhole_i18n::t("settings.subconscious.run_started");
                            view.tone = StatusTone::Success;
                        } else {
                            view.message = trigger.message.unwrap_or_else(|| {
                                wormhole_i18n::t("settings.subconscious.run_busy")
                            });
                            view.tone = StatusTone::Warn;
                        }
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.subconscious.run_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn set_mode(&mut self, mode: &str, ctx: &mut ViewContext<Self>) {
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        draft.mode = mode.to_string();
        if mode == "simple" && draft.interval_minutes == 5 {
            draft.interval_minutes = 30;
            self.interval_draft = "30".into();
        } else if mode == "aggressive" && draft.interval_minutes >= 30 {
            draft.interval_minutes = 5;
            self.interval_draft = "5".into();
        }
        self.draft = Some(draft);
        ctx.notify();
    }

    fn toggle_inference(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        draft.inference_enabled = !draft.inference_enabled;
        self.draft = Some(draft);
        ctx.notify();
    }

    fn action_button(
        &self,
        label: &str,
        action: SubconsciousAction,
        disabled: bool,
        primary: bool,
        automation_id: &str,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if disabled {
                        theme::placeholder()
                    } else if primary {
                        theme::bg()
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
        .with_padding_left(16.0)
        .with_padding_right(16.0)
        .with_padding_top(9.0)
        .with_padding_bottom(9.0)
        .with_background(if primary {
            if disabled {
                theme::accent_bg(20)
            } else {
                theme::accent()
            }
        } else {
            theme::accent_cool_bg(if disabled { 10 } else { 28 })
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
        .with_border(Border::all(1.0).with_border_fill(if primary {
            theme::accent()
        } else {
            theme::border()
        }))
        .finish()
    }

    fn mode_chip(&self, mode: &'static str, selected: bool) -> Box<dyn Element> {
        let label = mode_label(mode);
        let disabled = self.busy;
        EventHandler::new(
            Container::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if disabled {
                        theme::placeholder()
                    } else if selected {
                        theme::bg()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .with_background(if selected {
                theme::accent_cool()
            } else {
                theme::canvas()
            })
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .with_border(Border::all(1.0).with_border_fill(if selected {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .finish(),
        )
        .with_automation_label(label)
        .with_automation_id(format!("settings:subconscious_mode_{mode}"))
        .on_left_mouse_down(move |ctx, _, _| {
            if !disabled {
                ctx.dispatch_typed_action(SubconsciousAction::SetMode(mode));
            }
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn mode_selector(&self, current: &str) -> Box<dyn Element> {
        let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        for (i, mode) in ["off", "simple", "aggressive"].into_iter().enumerate() {
            let chip = self.mode_chip(mode, current == mode);
            if i == 0 {
                row.add_child(chip);
            } else {
                row.add_child(
                    Container::new(chip)
                        .with_margin_left(8.0)
                        .finish(),
                );
            }
        }
        row.finish()
    }

    fn toggle_row(&self, on: bool) -> Box<dyn Element> {
        let on_label = if on {
            wormhole_i18n::t("settings.subconscious.on")
        } else {
            wormhole_i18n::t("settings.subconscious.off")
        };
        let title = wormhole_i18n::t("settings.subconscious.inference");
        EventHandler::new(
            Container::new(
                Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(
                        Flex::row()
                            .with_main_axis_size(MainAxisSize::Max)
                            .with_child(
                                Expanded::new(
                                    1.0,
                                    ui_text::body(title.clone(), self.font)
                                        .with_color(theme::text())
                                        .finish(),
                                )
                                .finish(),
                            )
                            .with_child(
                                ui_text::body(on_label, self.font)
                                    .with_color(if on {
                                        theme::success()
                                    } else {
                                        theme::muted()
                                    })
                                    .finish(),
                            )
                            .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::body(
                                wormhole_i18n::t("settings.subconscious.inference_hint"),
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        )
                        .with_margin_top(4.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(14.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_automation_label(title)
        .with_automation_id("settings:subconscious_inference")
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(SubconsciousAction::ToggleInference);
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn interval_field(&self) -> Box<dyn Element> {
        let placeholder = wormhole_i18n::t("settings.subconscious.interval_placeholder");
        let field = render_field_with_caret(
            &self.interval_draft,
            &self.interval_field.marked_text,
            placeholder.as_str(),
            self.font,
            self.interval_focused,
            false,
            false,
            self.interval_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(SubconsciousAction::IntervalEdit(action));
        })
        .focused(self.interval_focused)
        .ime_preedit(!self.interval_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(SubconsciousAction::FocusInterval);
        });
        EventHandler::new(
            Container::new(input)
                .with_uniform_padding(10.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
                .finish(),
        )
        .with_automation_id("settings:subconscious_interval")
        .with_automation_label(wormhole_i18n::t("settings.subconscious.interval"))
        .finish()
    }

    fn meta_tile(&self, label: &str, value: &str) -> Box<dyn Element> {
        Container::new(
            Flex::column()
                .with_child(
                    ui_text::body(label.to_string(), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_child(
                    Container::new(
                        ui_text::body(value.to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_margin_top(4.0)
                    .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(12.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
    }

    fn status_state_label(&self, st: &SubconsciousStatus) -> (String, StatusTone) {
        if st.tick_in_progress {
            return (
                wormhole_i18n::t("settings.subconscious.state_running"),
                StatusTone::Warn,
            );
        }
        if !st.enabled {
            return (
                wormhole_i18n::t("settings.subconscious.state_off"),
                StatusTone::Muted,
            );
        }
        if st.consecutive_failures > 0 {
            return (
                wormhole_i18n::t("settings.subconscious.state_degraded"),
                StatusTone::Warn,
            );
        }
        (
            wormhole_i18n::t("settings.subconscious.state_idle"),
            StatusTone::Success,
        )
    }

    fn status_card(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.subconscious.status_heading"),
            self.font,
        ));

        let Some(st) = &self.status else {
            col.add_child(
                Container::new(status_line(
                    wormhole_i18n::t("settings.subconscious.status_unknown"),
                    self.font,
                    StatusTone::Placeholder,
                ))
                .with_margin_top(8.0)
                .finish(),
            );
            return section_card(col.finish());
        };

        let (state_label, state_tone) = self.status_state_label(st);
        col.add_child(
            Container::new(status_line(state_label, self.font, state_tone))
                .with_margin_top(8.0)
                .finish(),
        );

        let last_line = wormhole_i18n::t_args(
            "settings.subconscious.last_run",
            &[("when", &format_relative_ago(st.last_tick_at_ms))],
        );
        col.add_child(
            Container::new(
                ui_text::body(last_line, self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(6.0)
            .finish(),
        );

        let summary = st
            .last_summary
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| wormhole_i18n::t("settings.subconscious.summary_empty"));
        col.add_child(
            Container::new(
                Flex::column()
                    .with_child(
                        ui_text::body(
                            wormhole_i18n::t("settings.subconscious.last_summary"),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::body(summary, self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .with_margin_top(4.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_margin_top(10.0)
            .with_uniform_padding(12.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        );

        let mut meta = Flex::row().with_main_axis_size(MainAxisSize::Max);
        meta.add_child(Expanded::new(
            1.0,
            self.meta_tile(
                &wormhole_i18n::t("settings.subconscious.meta_ticks"),
                &st.total_ticks.to_string(),
            ),
        )
        .finish());
        meta.add_child(
            Container::new(Expanded::new(
                1.0,
                self.meta_tile(
                    &wormhole_i18n::t("settings.subconscious.meta_failures"),
                    &st.consecutive_failures.to_string(),
                ),
            )
            .finish())
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(meta.finish())
                .with_margin_top(10.0)
                .finish(),
        );

        section_card(col.finish())
    }

    fn schedule_card(&self, draft: &Draft) -> Box<dyn Element> {
        let mut body = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        body.add_child(section_title(
            wormhole_i18n::t("settings.subconscious.schedule_heading"),
            self.font,
        ));

        body.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("settings.subconscious.mode"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
        body.add_child(
            EventHandler::new(
                Container::new(self.mode_selector(&draft.mode))
                    .with_margin_top(8.0)
                    .finish(),
            )
            .with_automation_id("settings:subconscious_mode")
            .with_automation_label(wormhole_i18n::t("settings.subconscious.mode"))
            .finish(),
        );
        body.add_child(
            Container::new(
                ui_text::body(mode_hint(&draft.mode), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(6.0)
            .finish(),
        );

        body.add_child(
            Container::new(self.toggle_row(draft.inference_enabled))
                .with_margin_top(14.0)
                .finish(),
        );

        body.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("settings.subconscious.interval"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_top(14.0)
            .finish(),
        );
        body.add_child(
            Container::new(self.interval_field())
                .with_margin_top(8.0)
                .finish(),
        );
        body.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("settings.subconscious.interval_hint"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_top(6.0)
            .finish(),
        );

        section_card(body.finish())
    }
}

impl Entity for SubconsciousView {
    type Event = ();
}

impl View for SubconsciousView {
    fn ui_name() -> &'static str {
        "SubconsciousView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.subconscious.hint"),
            self.font,
        ));
        col.add_child(
            Container::new(self.status_card())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        if let Some(draft) = &self.draft {
            col.add_child(
                Container::new(self.schedule_card(draft))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            );

            let mut actions = Flex::row();
            actions.add_child(self.action_button(
                &wormhole_i18n::t("settings.subconscious.run_now"),
                SubconsciousAction::RunNow,
                self.busy,
                true,
                "settings:subconscious_run_now",
            ));
            actions.add_child(
                Container::new(self.action_button(
                    &wormhole_i18n::t("settings.subconscious.save"),
                    SubconsciousAction::Save,
                    self.busy,
                    false,
                    "settings:subconscious_save",
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            actions.add_child(
                Container::new(self.action_button(
                    &wormhole_i18n::t("settings.subconscious.refresh"),
                    SubconsciousAction::Refresh,
                    self.busy,
                    false,
                    "settings:subconscious_refresh",
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            col.add_child(
                Container::new(actions.finish())
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            );
        }

        if let Some(st) = &self.status {
            if let Some(err) = &st.last_error {
                col.add_child(
                    Container::new(status_line(
                        format!(
                            "{}: {err}",
                            wormhole_i18n::t("settings.subconscious.last_error")
                        ),
                        self.font,
                        StatusTone::Danger,
                    ))
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
            if let Some(reason) = &st.provider_unavailable_reason {
                col.add_child(
                    Container::new(status_line(
                        format!(
                            "{}: {reason}",
                            wormhole_i18n::t("settings.subconscious.provider")
                        ),
                        self.font,
                        StatusTone::Warn,
                    ))
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
        }

        if !self.message.is_empty() {
            col.add_child(
                EventHandler::new(
                    Container::new(status_line(self.message.clone(), self.font, self.tone))
                        .with_margin_top(12.0)
                        .finish(),
                )
                .with_automation_id("settings:subconscious_status")
                .with_automation_label(wormhole_i18n::t("settings.subconscious.status_label"))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            );
        }

        col.finish()
    }
}

impl TypedActionView for SubconsciousView {
    type Action = SubconsciousAction;

    fn handle_action(&mut self, action: &SubconsciousAction, ctx: &mut ViewContext<Self>) {
        match action {
            SubconsciousAction::Refresh => self.refresh(ctx),
            SubconsciousAction::Save => self.save(ctx),
            SubconsciousAction::RunNow => self.run_now(ctx),
            SubconsciousAction::SetMode(mode) => self.set_mode(mode, ctx),
            SubconsciousAction::ToggleInference => self.toggle_inference(ctx),
            SubconsciousAction::FocusInterval => {
                self.interval_focused = true;
                ctx.notify();
            }
            SubconsciousAction::IntervalEdit(edit) => {
                self.interval_field.apply(&mut self.interval_draft, edit);
                ctx.notify();
            }
        }
    }
}
