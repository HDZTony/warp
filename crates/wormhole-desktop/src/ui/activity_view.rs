//! Settings → Activity: Automations / Background / Alerts hub.

use warpui::elements::{
    Border, ChildView, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};
use wormhole_desktop_core::notification_commands::{
    notification_clear, notification_core_list, notification_core_mark_read, notification_list,
    notification_mark_all_read, notification_mark_read, notification_settings_list,
    notification_settings_set, NotificationCoreListParams, NotificationIdParams,
    NotificationListParams,
};
use wormhole_notifications::{
    CoreNotificationEvent, IntegrationNotification, NotificationSettings,
    NotificationSettingsUpsertRequest,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::cron_view::CronView;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::subconscious_view::SubconsciousView;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityTab {
    Automations,
    Background,
    Alerts,
}

#[derive(Debug, Clone)]
pub enum ActivityAction {
    SelectTab(u8),
    RefreshAlerts,
    MarkAllRead,
    Clear,
    OpenIntegration(String),
    OpenCore(String),
    ToggleProviderEnabled(String),
    ToggleProviderRoute(String),
    CycleProviderThreshold(String),
}

pub struct ActivityView {
    core: CoreHandle,
    font: FamilyId,
    tab: ActivityTab,
    cron: ViewHandle<CronView>,
    subconscious: ViewHandle<SubconsciousView>,
    integration_items: Vec<IntegrationNotification>,
    core_items: Vec<CoreNotificationEvent>,
    settings: Vec<NotificationSettings>,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl ActivityView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let cron = ctx.add_typed_action_view(|ctx| CronView::new(ctx, core.clone()));
        let subconscious =
            ctx.add_typed_action_view(|ctx| SubconsciousView::new(ctx, core.clone()));
        let mut view = Self {
            core,
            font,
            tab: ActivityTab::Alerts,
            cron,
            subconscious,
            integration_items: Vec::new(),
            core_items: Vec::new(),
            settings: Vec::new(),
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh_alerts(ctx);
        view
    }

    fn refresh_alerts(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.activity.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let list = notification_list(
                    &state,
                    NotificationListParams {
                        provider: None,
                        limit: Some(40),
                        offset: Some(0),
                        min_score: None,
                    },
                )
                .await?;
                let core_list = notification_core_list(
                    &state,
                    NotificationCoreListParams {
                        only_unread: Some(false),
                        limit: Some(40),
                    },
                )
                .await?;
                let settings = notification_settings_list(&state).await?;
                Ok::<_, String>((list, core_list, settings))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((list, core_list, settings)) => {
                        view.integration_items = list.items;
                        view.core_items = core_list.items;
                        view.settings = settings;
                        view.message = wormhole_i18n::t("settings.activity.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.activity.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn mark_all_read(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                notification_mark_all_read(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(_) => {
                        view.message = wormhole_i18n::t("settings.activity.marked_all_read");
                        view.tone = StatusTone::Success;
                        view.refresh_alerts(ctx);
                    }
                    Err(error) => {
                        view.message = error;
                        view.tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn clear_feed(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                notification_clear(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(_) => {
                        view.message = wormhole_i18n::t("settings.activity.cleared");
                        view.tone = StatusTone::Success;
                        view.refresh_alerts(ctx);
                    }
                    Err(error) => {
                        view.message = error;
                        view.tone = StatusTone::Danger;
                        ctx.notify();
                    }
                }
            },
        );
    }

    fn open_integration(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                notification_mark_read(&state, NotificationIdParams { id }).await
            },
            |view, _, ctx| view.refresh_alerts(ctx),
        );
    }

    fn open_core(&mut self, id: String, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                notification_core_mark_read(&state, NotificationIdParams { id }).await
            },
            |view, _, ctx| view.refresh_alerts(ctx),
        );
    }

    fn upsert_setting(&mut self, settings: NotificationSettings, ctx: &mut ViewContext<Self>) {
        let core = self.core.clone();
        let req = NotificationSettingsUpsertRequest {
            provider: settings.provider.clone(),
            enabled: settings.enabled,
            importance_threshold: settings.importance_threshold,
            route_to_orchestrator: settings.route_to_orchestrator,
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                notification_settings_set(&state, req).await
            },
            |view, output, ctx| {
                match output {
                    Ok(s) => {
                        if let Some(slot) =
                            view.settings.iter_mut().find(|x| x.provider == s.provider)
                        {
                            *slot = s;
                        } else {
                            view.settings.push(s);
                        }
                        view.message = wormhole_i18n::t("settings.activity.settings_saved");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = error;
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn action_chip(
        &self,
        label: &str,
        action: ActivityAction,
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

    fn render_alerts(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.activity.alerts_title"),
            self.font,
        ));
        col.add_child(
            Container::new(section_hint(
                wormhole_i18n::t("settings.activity.alerts_hint"),
                self.font,
            ))
            .with_margin_top(6.0)
            .finish(),
        );

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.action_chip(
            &wormhole_i18n::t("settings.activity.refresh"),
            ActivityAction::RefreshAlerts,
            false,
            "settings:activity_refresh",
            self.busy,
        ));
        actions.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.activity.mark_all_read"),
                ActivityAction::MarkAllRead,
                false,
                "settings:activity_mark_all_read",
                self.busy,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.activity.clear"),
                ActivityAction::Clear,
                false,
                "settings:activity_clear",
                self.busy,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(10.0)
                .finish(),
        );
        col.add_child(
            Container::new(status_line(self.message.clone(), self.font, self.tone))
                .with_margin_top(8.0)
                .finish(),
        );

        let mut core_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        core_col.add_child(section_title(
            wormhole_i18n::t("settings.activity.core_stream"),
            self.font,
        ));
        if self.core_items.is_empty() {
            core_col.add_child(
                Container::new(
                    ui_text::body(wormhole_i18n::t("settings.activity.empty_core"), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for item in self.core_items.iter().take(20) {
                let id = item.id.clone();
                let title = format!("[{}] {}", item.category.as_str(), item.title);
                let body = item.body.clone();
                let mut card = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                card.add_child(
                    EventHandler::new(
                        ui_text::body(title, self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ActivityAction::OpenCore(id.clone()));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                );
                card.add_child(
                    Container::new(
                        ui_text::body(body, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(4.0)
                    .finish(),
                );
                core_col.add_child(
                    Container::new(section_card(card.finish()))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
        }
        col.add_child(
            Container::new(section_card(core_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut integ = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        integ.add_child(section_title(
            wormhole_i18n::t("settings.activity.integration_stream"),
            self.font,
        ));
        if self.integration_items.is_empty() {
            integ.add_child(
                Container::new(
                    ui_text::body(
                        wormhole_i18n::t("settings.activity.empty_integration"),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for item in self.integration_items.iter().take(20) {
                let id = item.id.clone();
                let score = item
                    .importance_score
                    .map(|s| format!("{s:.2}"))
                    .unwrap_or_else(|| "—".into());
                let action = item
                    .triage_action
                    .clone()
                    .unwrap_or_else(|| "pending".into());
                let meta = format!(
                    "{} · {} · score {score} · {action}",
                    item.provider,
                    item.status.as_str()
                );
                let mut card = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
                card.add_child(
                    EventHandler::new(
                        ui_text::body(meta, self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ActivityAction::OpenIntegration(id.clone()));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                );
                card.add_child(
                    Container::new(
                        ui_text::body(item.title.clone(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_margin_top(4.0)
                    .finish(),
                );
                card.add_child(
                    Container::new(
                        ui_text::body(item.body.clone(), self.font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_margin_top(2.0)
                    .finish(),
                );
                if let Some(reason) = item.triage_reason.as_ref() {
                    card.add_child(
                        Container::new(
                            ui_text::body(reason.clone(), self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .with_margin_top(2.0)
                        .finish(),
                    );
                }
                integ.add_child(
                    Container::new(section_card(card.finish()))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
        }
        col.add_child(
            Container::new(section_card(integ.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut routing = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        routing.add_child(section_title(
            wormhole_i18n::t("settings.activity.routing_title"),
            self.font,
        ));
        routing.add_child(
            Container::new(section_hint(
                wormhole_i18n::t("settings.activity.routing_hint"),
                self.font,
            ))
            .with_margin_top(6.0)
            .finish(),
        );
        for s in &self.settings {
            let provider = s.provider.clone();
            let enabled_label = if s.enabled {
                format!("{} · ON", s.provider)
            } else {
                format!("{} · OFF", s.provider)
            };
            let route_label = if s.route_to_orchestrator {
                wormhole_i18n::t("settings.activity.route_on")
            } else {
                wormhole_i18n::t("settings.activity.route_off")
            };
            let threshold_label = format!("threshold {:.2}", s.importance_threshold);
            let mut row = Flex::row().with_main_axis_size(MainAxisSize::Max);
            let p1 = provider.clone();
            let auto_id = format!("settings:activity_provider_{}_enabled", s.provider);
            row.add_child(self.action_chip(
                &enabled_label,
                ActivityAction::ToggleProviderEnabled(p1),
                false,
                &auto_id,
                false,
            ));
            let p2 = provider.clone();
            row.add_child(
                Container::new(self.action_chip(
                    &route_label,
                    ActivityAction::ToggleProviderRoute(p2),
                    false,
                    "settings:activity_provider_route",
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            let p3 = provider;
            row.add_child(
                Container::new(self.action_chip(
                    &threshold_label,
                    ActivityAction::CycleProviderThreshold(p3),
                    false,
                    "settings:activity_provider_threshold",
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            routing.add_child(
                Container::new(section_card(row.finish()))
                    .with_margin_top(8.0)
                    .finish(),
            );
        }
        col.add_child(
            Container::new(section_card(routing.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        col.finish()
    }
}

impl Entity for ActivityView {
    type Event = ();
}

impl View for ActivityView {
    fn ui_name() -> &'static str {
        "ActivityView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut root = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        root.add_child(section_title(
            wormhole_i18n::t("settings.activity.title"),
            self.font,
        ));
        root.add_child(
            Container::new(section_hint(
                wormhole_i18n::t("settings.activity.hint"),
                self.font,
            ))
            .with_margin_top(6.0)
            .finish(),
        );

        let mut tabs = Flex::row().with_main_axis_size(MainAxisSize::Max);
        tabs.add_child(self.action_chip(
            &wormhole_i18n::t("settings.activity.tab_automations"),
            ActivityAction::SelectTab(0),
            self.tab == ActivityTab::Automations,
            "settings:activity_tab_automations",
            false,
        ));
        tabs.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.activity.tab_background"),
                ActivityAction::SelectTab(1),
                self.tab == ActivityTab::Background,
                "settings:activity_tab_background",
                false,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        tabs.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.activity.tab_alerts"),
                ActivityAction::SelectTab(2),
                self.tab == ActivityTab::Alerts,
                "settings:activity_tab_alerts",
                false,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        root.add_child(
            Container::new(tabs.finish())
                .with_margin_top(12.0)
                .finish(),
        );

        match self.tab {
            ActivityTab::Automations => {
                root.add_child(
                    Container::new(ChildView::new(&self.cron).finish())
                        .with_margin_top(SECTION_GAP)
                        .finish(),
                );
            }
            ActivityTab::Background => {
                root.add_child(
                    Container::new(ChildView::new(&self.subconscious).finish())
                        .with_margin_top(SECTION_GAP)
                        .finish(),
                );
            }
            ActivityTab::Alerts => {
                root.add_child(
                    Container::new(self.render_alerts())
                        .with_margin_top(SECTION_GAP)
                        .finish(),
                );
            }
        }

        EventHandler::new(root.finish())
            .with_automation_id("settings:activity_root")
            .finish()
    }
}

impl TypedActionView for ActivityView {
    type Action = ActivityAction;

    fn handle_action(&mut self, action: &ActivityAction, ctx: &mut ViewContext<Self>) {
        match action {
            ActivityAction::SelectTab(0) => {
                self.tab = ActivityTab::Automations;
                ctx.notify();
            }
            ActivityAction::SelectTab(1) => {
                self.tab = ActivityTab::Background;
                ctx.notify();
            }
            ActivityAction::SelectTab(_) => {
                self.tab = ActivityTab::Alerts;
                self.refresh_alerts(ctx);
            }
            ActivityAction::RefreshAlerts => self.refresh_alerts(ctx),
            ActivityAction::MarkAllRead => self.mark_all_read(ctx),
            ActivityAction::Clear => self.clear_feed(ctx),
            ActivityAction::OpenIntegration(id) => self.open_integration(id.clone(), ctx),
            ActivityAction::OpenCore(id) => self.open_core(id.clone(), ctx),
            ActivityAction::ToggleProviderEnabled(provider) => {
                if let Some(s) = self.settings.iter().find(|x| x.provider == *provider).cloned() {
                    let mut next = s;
                    next.enabled = !next.enabled;
                    self.upsert_setting(next, ctx);
                }
            }
            ActivityAction::ToggleProviderRoute(provider) => {
                if let Some(s) = self.settings.iter().find(|x| x.provider == *provider).cloned() {
                    let mut next = s;
                    next.route_to_orchestrator = !next.route_to_orchestrator;
                    self.upsert_setting(next, ctx);
                }
            }
            ActivityAction::CycleProviderThreshold(provider) => {
                if let Some(s) = self.settings.iter().find(|x| x.provider == *provider).cloned() {
                    let mut next = s;
                    let steps = [0.0_f32, 0.35, 0.65, 0.90];
                    let idx = steps
                        .iter()
                        .position(|v| (*v - next.importance_threshold).abs() < 0.01)
                        .unwrap_or(0);
                    next.importance_threshold = steps[(idx + 1) % steps.len()];
                    self.upsert_setting(next, ctx);
                }
            }
        }
    }
}
