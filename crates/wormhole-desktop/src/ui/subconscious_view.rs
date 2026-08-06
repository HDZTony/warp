//! Settings → Subconscious: background memory-world reflection loop.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
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
    CycleMode,
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

    fn cycle_mode(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        draft.mode = match draft.mode.as_str() {
            "off" => "simple".into(),
            "simple" => "aggressive".into(),
            _ => "off".into(),
        };
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
                .with_uniform_padding(8.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
        )
        .with_automation_id("settings:subconscious_interval")
        .with_automation_label(wormhole_i18n::t("settings.subconscious.interval"))
        .finish()
    }

    fn status_summary(&self) -> String {
        let Some(st) = &self.status else {
            return wormhole_i18n::t("settings.subconscious.status_unknown");
        };
        let last = st
            .last_tick_at_ms
            .map(|ms| format!("{}s", ms / 1000))
            .unwrap_or_else(|| wormhole_i18n::t("settings.subconscious.never"));
        let summary = st.last_summary.clone().unwrap_or_else(|| "—".into());
        format!(
            "{} · ticks {} · failures {} · last {} · {}",
            st.mode, st.total_ticks, st.consecutive_failures, last, summary
        )
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
        let draft = self.draft.clone();
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.subconscious.hint"),
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP)
                .finish(),
        );

        let status_tone = if self
            .status
            .as_ref()
            .map(|s| s.consecutive_failures > 0)
            .unwrap_or(false)
        {
            StatusTone::Warn
        } else {
            StatusTone::Placeholder
        };
        col.add_child(section_title(
            wormhole_i18n::t("settings.subconscious.title"),
            self.font,
        ));
        col.add_child(status_line(self.status_summary(), self.font, status_tone));

        if let Some(draft) = draft {
            let mode_label = format!(
                "{}: {}",
                wormhole_i18n::t("settings.subconscious.mode"),
                draft.mode
            );
            let inference_label = format!(
                "{}: {}",
                wormhole_i18n::t("settings.subconscious.inference"),
                if draft.inference_enabled {
                    wormhole_i18n::t("settings.subconscious.on")
                } else {
                    wormhole_i18n::t("settings.subconscious.off")
                }
            );

            let mut body = Flex::column();
            body.add_child(
                ui_text::body(
                    wormhole_i18n::t("settings.subconscious.interval"),
                    self.font,
                )
                .finish(),
            );
            body.add_child(
                Container::new(self.interval_field())
                    .with_margin_top(8.0)
                    .finish(),
            );

            let mut controls = Flex::row();
            controls.add_child(self.action_button(
                &mode_label,
                SubconsciousAction::CycleMode,
                self.busy,
                false,
                "settings:subconscious_mode",
            ));
            controls.add_child(
                Container::new(self.action_button(
                    &inference_label,
                    SubconsciousAction::ToggleInference,
                    self.busy,
                    false,
                    "settings:subconscious_inference",
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            body.add_child(
                Container::new(controls.finish())
                    .with_margin_top(10.0)
                    .finish(),
            );

            let mut actions = Flex::row();
            actions.add_child(self.action_button(
                &wormhole_i18n::t("settings.subconscious.save"),
                SubconsciousAction::Save,
                self.busy,
                true,
                "settings:subconscious_save",
            ));
            actions.add_child(
                Container::new(self.action_button(
                    &wormhole_i18n::t("settings.subconscious.run_now"),
                    SubconsciousAction::RunNow,
                    self.busy,
                    true,
                    "settings:subconscious_run_now",
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
            body.add_child(
                Container::new(actions.finish())
                    .with_margin_top(10.0)
                    .finish(),
            );

            col.add_child(
                Container::new(section_card(body.finish()))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            );
        }

        if let Some(st) = &self.status {
            if let Some(err) = &st.last_error {
                col.add_child(
                    Container::new(status_line(err.clone(), self.font, StatusTone::Danger))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
            if let Some(reason) = &st.provider_unavailable_reason {
                col.add_child(
                    Container::new(status_line(reason.clone(), self.font, StatusTone::Warn))
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
            SubconsciousAction::CycleMode => self.cycle_mode(ctx),
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
