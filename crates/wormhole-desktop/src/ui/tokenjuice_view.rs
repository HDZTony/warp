//! Settings → TokenJuice: tool-output compression savings and router toggles.

use std::collections::HashMap;

use serde::Deserialize;
use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::tokenjuice_commands::{
    tokenjuice_savings_reset, tokenjuice_savings_stats, tokenjuice_settings_get,
    tokenjuice_settings_update, TokenjuiceSettingsPatchDto, TokenjuiceSettingsUpdateParams,
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavingsBucket {
    events: u64,
    tokens_saved: u64,
    cost_saved_usd: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheStats {
    entries: usize,
    bytes: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavingsStats {
    attribution_model: String,
    total: SavingsBucket,
    by_compressor: HashMap<String, SavingsBucket>,
    cache: CacheStats,
}

#[derive(Debug, Clone, Default)]
struct SettingsDraft {
    router_enabled: bool,
    search_enabled: bool,
    code_enabled: bool,
    html_enabled: bool,
    ml_compression_enabled: bool,
    ccr_enabled: bool,
    ccr_disk_enabled: bool,
    ccr_min_tokens: usize,
}

impl SettingsDraft {
    fn from_json(value: serde_json::Value) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            router_enabled: bool,
            search_enabled: bool,
            code_enabled: bool,
            html_enabled: bool,
            ml_compression_enabled: bool,
            ccr_enabled: bool,
            ccr_disk_enabled: bool,
            ccr_min_tokens: usize,
        }
        let raw: Raw = serde_json::from_value(value).map_err(|e| e.to_string())?;
        Ok(Self {
            router_enabled: raw.router_enabled,
            search_enabled: raw.search_enabled,
            code_enabled: raw.code_enabled,
            html_enabled: raw.html_enabled,
            ml_compression_enabled: raw.ml_compression_enabled,
            ccr_enabled: raw.ccr_enabled,
            ccr_disk_enabled: raw.ccr_disk_enabled,
            ccr_min_tokens: raw.ccr_min_tokens,
        })
    }

    fn to_patch(&self) -> TokenjuiceSettingsPatchDto {
        TokenjuiceSettingsPatchDto {
            router_enabled: Some(self.router_enabled),
            search_enabled: Some(self.search_enabled),
            code_enabled: Some(self.code_enabled),
            html_enabled: Some(self.html_enabled),
            ml_compression_enabled: Some(self.ml_compression_enabled),
            ccr_enabled: Some(self.ccr_enabled),
            ccr_disk_enabled: Some(self.ccr_disk_enabled),
            ccr_min_tokens: Some(self.ccr_min_tokens),
            max_cache_entries: None,
            max_cache_bytes: None,
            ccr_ttl_secs: None,
            min_bytes_to_compress: None,
            ml_model_id: None,
            ml_target_ratio: None,
            ml_sidecar_idle_timeout_secs: None,
            ml_max_input_chars: None,
            ml_device: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToggleField {
    Router,
    Search,
    Code,
    Html,
    Ml,
    Ccr,
    CcrDisk,
}

#[derive(Debug, Clone)]
pub enum TokenJuiceAction {
    RefreshStats,
    ResetSavings,
    SaveSettings,
    Toggle(ToggleField),
    FocusMinTokens,
    MinTokensEdit(TextFieldEditAction),
}

pub struct TokenJuiceView {
    core: CoreHandle,
    font: FamilyId,
    draft: Option<SettingsDraft>,
    savings: Option<SavingsStats>,
    min_tokens_draft: String,
    min_tokens_field: TextFieldState,
    min_tokens_focused: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

fn format_int(n: u64) -> String {
    n.to_string()
}

fn format_usd(n: f64) -> String {
    if n > 0.0 && n < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", n)
    }
}

fn format_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

impl TokenJuiceView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            draft: None,
            savings: None,
            min_tokens_draft: String::new(),
            min_tokens_field: TextFieldState::new(),
            min_tokens_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh_all(ctx);
        view
    }

    fn apply_draft(&mut self, draft: SettingsDraft) {
        self.min_tokens_draft = draft.ccr_min_tokens.to_string();
        self.min_tokens_field = TextFieldState::new();
        self.draft = Some(draft);
    }

    fn refresh_all(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.tokenjuice.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let settings = tokenjuice_settings_get(&state).await?;
                let settings_json =
                    serde_json::to_value(&settings).map_err(|e| e.to_string())?;
                let savings_raw = tokenjuice_savings_stats(&state).await?;
                let savings: SavingsStats =
                    serde_json::from_value(savings_raw).map_err(|e| e.to_string())?;
                Ok::<_, String>((settings_json, savings))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((settings_json, savings)) => {
                        match SettingsDraft::from_json(settings_json) {
                            Ok(draft) => {
                                view.apply_draft(draft);
                                view.savings = Some(savings);
                                if view.message == wormhole_i18n::t("settings.tokenjuice.loading")
                                {
                                    view.message = wormhole_i18n::t("settings.tokenjuice.ready");
                                    view.tone = StatusTone::Placeholder;
                                }
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.tokenjuice.load_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.tokenjuice.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_stats(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let savings_raw = tokenjuice_savings_stats(&state).await?;
                serde_json::from_value::<SavingsStats>(savings_raw).map_err(|e| e.to_string())
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(savings) => {
                        view.savings = Some(savings);
                        view.message = wormhole_i18n::t("settings.tokenjuice.stats_refreshed");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.tokenjuice.stats_refresh_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn save_settings(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let parsed_min = self.min_tokens_draft.trim().parse::<usize>().unwrap_or(draft.ccr_min_tokens);
        let mut to_save = draft;
        to_save.ccr_min_tokens = parsed_min;
        self.busy = true;
        self.message = wormhole_i18n::t("settings.tokenjuice.saving");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let patch = to_save.to_patch();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let cfg = tokenjuice_settings_update(
                    &state,
                    TokenjuiceSettingsUpdateParams { patch },
                )
                .await?;
                let json = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
                Ok::<_, String>(json)
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(json) => match SettingsDraft::from_json(json) {
                        Ok(draft) => {
                            view.apply_draft(draft);
                            view.message = wormhole_i18n::t("settings.tokenjuice.saved");
                            view.tone = StatusTone::Success;
                        }
                        Err(error) => {
                            view.message = format!(
                                "{}: {error}",
                                wormhole_i18n::t("settings.tokenjuice.save_failed")
                            );
                            view.tone = StatusTone::Danger;
                        }
                    },
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.tokenjuice.save_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn reset_savings(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                tokenjuice_savings_reset(&state).await?;
                let savings_raw = tokenjuice_savings_stats(&state).await?;
                serde_json::from_value::<SavingsStats>(savings_raw).map_err(|e| e.to_string())
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(savings) => {
                        view.savings = Some(savings);
                        view.message = wormhole_i18n::t("settings.tokenjuice.reset_done");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.tokenjuice.reset_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn toggle_field(&mut self, field: ToggleField, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let Some(mut draft) = self.draft.clone() else {
            return;
        };
        match field {
            ToggleField::Router => draft.router_enabled = !draft.router_enabled,
            ToggleField::Search => draft.search_enabled = !draft.search_enabled,
            ToggleField::Code => draft.code_enabled = !draft.code_enabled,
            ToggleField::Html => draft.html_enabled = !draft.html_enabled,
            ToggleField::Ml => draft.ml_compression_enabled = !draft.ml_compression_enabled,
            ToggleField::Ccr => draft.ccr_enabled = !draft.ccr_enabled,
            ToggleField::CcrDisk => draft.ccr_disk_enabled = !draft.ccr_disk_enabled,
        }
        self.draft = Some(draft);
        ctx.notify();
    }

    fn action_button(
        &self,
        label: &str,
        action: TokenJuiceAction,
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

    fn toggle_row(
        &self,
        label: &str,
        description: &str,
        on: bool,
        field: ToggleField,
        automation_id: &str,
    ) -> Box<dyn Element> {
        let row_label = label.to_string();
        let on_label = if on {
            wormhole_i18n::t("settings.tokenjuice.on")
        } else {
            wormhole_i18n::t("settings.tokenjuice.off")
        };
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
                                    ui_text::body(row_label.clone(), self.font)
                                        .with_color(theme::text())
                                        .finish(),
                                )
                                .finish(),
                            )
                            .with_child(
                                ui_text::body(on_label, self.font)
                                    .with_color(theme::accent_cool())
                                    .finish(),
                            )
                            .finish(),
                    )
                    .with_child(
                        ui_text::body(description.to_string(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .finish(),
            )
            .with_uniform_padding(14.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
        )
        .with_automation_label(row_label)
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(TokenJuiceAction::Toggle(field));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn stat_tile(&self, label: &str, value: &str, hint: Option<&str>) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        col.add_child(
            ui_text::hud_title(value.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if let Some(hint) = hint {
            col.add_child(
                ui_text::body(hint.to_string(), self.font)
                    .with_color(theme::placeholder())
                    .finish(),
            );
        }
        Container::new(col.finish())
            .with_uniform_padding(12.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .finish()
    }

    fn min_tokens_field(&self) -> Box<dyn Element> {
        let placeholder = wormhole_i18n::t("settings.tokenjuice.ccr_min_tokens_placeholder");
        let field = render_field_with_caret(
            &self.min_tokens_draft,
            &self.min_tokens_field.marked_text,
            placeholder.as_str(),
            self.font,
            self.min_tokens_focused,
            false,
            false,
            self.min_tokens_field.cursor,
        );
        let input = TextFieldInput::builder(field, |ctx, action| {
            ctx.dispatch_typed_action(TokenJuiceAction::MinTokensEdit(action));
        })
        .focused(self.min_tokens_focused)
        .ime_preedit(!self.min_tokens_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(TokenJuiceAction::FocusMinTokens);
        });
        EventHandler::new(
            Container::new(input)
                .with_uniform_padding(10.0)
                .with_background(theme::canvas())
                .with_border(Border::all(1.0).with_border_fill(if self.min_tokens_focused {
                    theme::accent_cool()
                } else {
                    theme::border()
                }))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
                .finish(),
        )
        .with_automation_id("settings:tokenjuice_min_tokens")
        .with_automation_label(wormhole_i18n::t("settings.tokenjuice.ccr_min_tokens"))
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
    }

    fn savings_section(&self) -> Box<dyn Element> {
        let mut col = Flex::column();
        col.add_child(section_title(
            wormhole_i18n::t("settings.tokenjuice.savings_title"),
            self.font,
        ));
        if let Some(savings) = &self.savings {
            let attributed = wormhole_i18n::t("settings.tokenjuice.attributed_to")
                .replace("{model}", &savings.attribution_model);
            col.add_child(section_hint(attributed, self.font));
        }
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );

        let total = self.savings.as_ref().map(|s| &s.total);
        let cache = self.savings.as_ref().map(|s| &s.cache);
        let events_hint = total.map(|t| {
            wormhole_i18n::t("settings.tokenjuice.over_events")
                .replace("{count}", &format_int(t.events))
        });
        let tokens_saved_value = total
            .map(|t| format_int(t.tokens_saved))
            .unwrap_or_else(|| "—".to_string());
        let cost_saved_value = total
            .map(|t| format_usd(t.cost_saved_usd))
            .unwrap_or_else(|| "—".to_string());
        let cache_entries_value = cache
            .map(|c| format_int(c.entries as u64))
            .unwrap_or_else(|| "—".to_string());
        let cache_bytes_hint = cache.map(|c| format_bytes(c.bytes));
        let events_value = total
            .map(|t| format_int(t.events))
            .unwrap_or_else(|| "—".to_string());
        let tokens_saved_label = wormhole_i18n::t("settings.tokenjuice.tokens_saved");
        let cost_saved_label = wormhole_i18n::t("settings.tokenjuice.cost_saved");
        let cache_label = wormhole_i18n::t("settings.tokenjuice.cache_occupancy");
        let events_label = wormhole_i18n::t("settings.tokenjuice.compactions");

        let mut stats_row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        stats_row.add_child(
            Expanded::new(
                1.0,
                self.stat_tile(
                    &tokens_saved_label,
                    &tokens_saved_value,
                    events_hint.as_deref(),
                ),
            )
            .finish(),
        );
        stats_row.add_child(
            Container::new(
                Expanded::new(
                    1.0,
                    self.stat_tile(&cost_saved_label, &cost_saved_value, None),
                )
                .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
        stats_row.add_child(
            Container::new(
                Expanded::new(
                    1.0,
                    self.stat_tile(
                        &cache_label,
                        &cache_entries_value,
                        cache_bytes_hint.as_deref(),
                    ),
                )
                .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
        stats_row.add_child(
            Container::new(
                Expanded::new(1.0, self.stat_tile(&events_label, &events_value, None))
                .finish(),
            )
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(stats_row.finish());

        if let Some(savings) = &self.savings {
            if !savings.by_compressor.is_empty() {
                col.add_child(
                    Container::new(
                        ui_text::body(
                            wormhole_i18n::t("settings.tokenjuice.by_compressor"),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_margin_top(10.0)
                    .finish(),
                );
                let mut by_col = Flex::column();
                let mut entries: Vec<_> = savings.by_compressor.iter().collect();
                entries.sort_by(|a, b| b.1.tokens_saved.cmp(&a.1.tokens_saved));
                for (name, bucket) in entries {
                    by_col.add_child(
                        Container::new(
                            Flex::row()
                                .with_main_axis_size(MainAxisSize::Max)
                                .with_child(
                                    Expanded::new(
                                        1.0,
                                        ui_text::mono(name.clone(), self.font)
                                            .with_color(theme::text())
                                            .finish(),
                                    )
                                    .finish(),
                                )
                                .with_child(
                                    ui_text::body(
                                        format!(
                                            "{} tok · {}",
                                            format_int(bucket.tokens_saved),
                                            format_usd(bucket.cost_saved_usd)
                                        ),
                                        self.font,
                                    )
                                    .with_color(theme::muted())
                                    .finish(),
                                )
                                .finish(),
                        )
                        .with_uniform_padding(10.0)
                        .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
                        .finish(),
                    );
                }
                col.add_child(
                    Container::new(by_col.finish())
                        .with_border(Border::all(1.0).with_border_fill(theme::border()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                        .finish(),
                );
            }
        }

        let mut actions = Flex::row();
        actions.add_child(self.action_button(
            &wormhole_i18n::t("settings.tokenjuice.refresh"),
            TokenJuiceAction::RefreshStats,
            self.busy,
            false,
            "settings:tokenjuice_refresh",
        ));
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.tokenjuice.reset"),
                TokenJuiceAction::ResetSavings,
                self.busy,
                false,
                "settings:tokenjuice_reset",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(10.0)
                .finish(),
        );

        section_card(col.finish())
    }

    fn compression_section(&self) -> Box<dyn Element> {
        let draft = self.draft.clone().unwrap_or_default();
        let mut col = Flex::column();
        col.add_child(section_title(
            wormhole_i18n::t("settings.tokenjuice.compression_title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.tokenjuice.compression_desc"),
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );
        let mut toggles = Flex::column();
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.router_enabled"),
                &wormhole_i18n::t("settings.tokenjuice.router_enabled_desc"),
                draft.router_enabled,
                ToggleField::Router,
                "settings:tokenjuice_router",
            ),
        );
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.search"),
                &wormhole_i18n::t("settings.tokenjuice.search_desc"),
                draft.search_enabled,
                ToggleField::Search,
                "settings:tokenjuice_search",
            ),
        );
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.code"),
                &wormhole_i18n::t("settings.tokenjuice.code_desc"),
                draft.code_enabled,
                ToggleField::Code,
                "settings:tokenjuice_code",
            ),
        );
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.html"),
                &wormhole_i18n::t("settings.tokenjuice.html_desc"),
                draft.html_enabled,
                ToggleField::Html,
                "settings:tokenjuice_html",
            ),
        );
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.ml"),
                &wormhole_i18n::t("settings.tokenjuice.ml_desc"),
                draft.ml_compression_enabled,
                ToggleField::Ml,
                "settings:tokenjuice_ml",
            ),
        );
        col.add_child(
            Container::new(toggles.finish())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
        );
        section_card(col.finish())
    }

    fn ccr_section(&self) -> Box<dyn Element> {
        let draft = self.draft.clone().unwrap_or_default();
        let mut col = Flex::column();
        col.add_child(section_title(
            wormhole_i18n::t("settings.tokenjuice.ccr_title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.tokenjuice.ccr_desc"),
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP / 2.0)
                .finish(),
        );
        let mut toggles = Flex::column();
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.ccr_enabled"),
                &wormhole_i18n::t("settings.tokenjuice.ccr_enabled_desc"),
                draft.ccr_enabled,
                ToggleField::Ccr,
                "settings:tokenjuice_ccr",
            ),
        );
        toggles.add_child(
            Container::new(
                Flex::column()
                    .with_child(
                        ui_text::body(
                            wormhole_i18n::t("settings.tokenjuice.ccr_min_tokens"),
                            self.font,
                        )
                        .with_color(theme::text())
                        .finish(),
                    )
                    .with_child(
                        ui_text::body(
                            wormhole_i18n::t("settings.tokenjuice.ccr_min_tokens_desc"),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_child(self.min_tokens_field())
                    .finish(),
            )
            .with_uniform_padding(14.0)
            .with_border(Border::bottom(1.0).with_border_fill(theme::border()))
            .finish(),
        );
        toggles.add_child(
            self.toggle_row(
                &wormhole_i18n::t("settings.tokenjuice.ccr_disk"),
                &wormhole_i18n::t("settings.tokenjuice.ccr_disk_desc"),
                draft.ccr_disk_enabled,
                ToggleField::CcrDisk,
                "settings:tokenjuice_ccr_disk",
            ),
        );
        col.add_child(
            Container::new(toggles.finish())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish(),
        );
        col.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.tokenjuice.save"),
                TokenJuiceAction::SaveSettings,
                self.busy || self.draft.is_none(),
                true,
                "settings:tokenjuice_save",
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        section_card(col.finish())
    }
}

impl Entity for TokenJuiceView {
    type Event = ();
}

impl View for TokenJuiceView {
    fn ui_name() -> &'static str {
        "TokenJuiceView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.tokenjuice.hint"),
            self.font,
        ));
        col.add_child(
            Container::new(Flex::column().finish())
                .with_vertical_margin(SECTION_GAP)
                .finish(),
        );
        col.add_child(self.savings_section());
        col.add_child(
            Container::new(self.compression_section())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );
        col.add_child(
            Container::new(self.ccr_section())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );
        if !self.message.is_empty() {
            col.add_child(
                EventHandler::new(
                    Container::new(status_line(self.message.clone(), self.font, self.tone))
                        .with_margin_top(12.0)
                        .finish(),
                )
                .with_automation_id("settings:tokenjuice_status")
                .with_automation_label(wormhole_i18n::t("settings.tokenjuice.status_label"))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            );
        }
        col.finish()
    }
}

impl TypedActionView for TokenJuiceView {
    type Action = TokenJuiceAction;

    fn handle_action(&mut self, action: &TokenJuiceAction, ctx: &mut ViewContext<Self>) {
        match action {
            TokenJuiceAction::RefreshStats => self.refresh_stats(ctx),
            TokenJuiceAction::ResetSavings => self.reset_savings(ctx),
            TokenJuiceAction::SaveSettings => self.save_settings(ctx),
            TokenJuiceAction::Toggle(field) => self.toggle_field(*field, ctx),
            TokenJuiceAction::FocusMinTokens => {
                self.min_tokens_focused = true;
                ctx.notify();
            }
            TokenJuiceAction::MinTokensEdit(edit) => {
                self.min_tokens_field.apply(&mut self.min_tokens_draft, edit);
                if let Some(draft) = self.draft.clone() {
                    if let Ok(parsed) = self.min_tokens_draft.trim().parse::<usize>() {
                        let mut next = draft;
                        next.ccr_min_tokens = parsed;
                        self.draft = Some(next);
                    }
                }
                ctx.notify();
            }
        }
    }
}
