//! Settings → Security: OS keyring status and consent controls.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::security::{
    keyring_consent_decide, keyring_consent_retry_probe, keyring_consent_status, KeyringStatus,
    StorageMode,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub enum SecurityAction {
    Refresh,
    RetryProbe,
    ConsentLocalEncrypted,
    ConsentDeclined,
}

pub struct SecurityView {
    #[allow(dead_code)]
    core: CoreHandle,
    font: FamilyId,
    status: KeyringStatus,
    message: String,
    tone: StatusTone,
}

impl SecurityView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let status = keyring_consent_status();
        Self {
            core,
            font,
            status,
            message: String::new(),
            tone: StatusTone::Neutral,
        }
    }

    fn refresh(&mut self) {
        self.status = keyring_consent_status();
        self.message.clear();
        self.tone = StatusTone::Neutral;
    }

    /// Called when Settings navigates to this page.
    pub fn reload(&mut self, ctx: &mut ViewContext<Self>) {
        self.refresh();
        ctx.notify();
    }

    fn mode_label(mode: StorageMode) -> String {
        match mode {
            StorageMode::OsKeyring => wormhole_i18n::t("settings.security.mode.os_keyring"),
            StorageMode::LocalEncrypted => {
                wormhole_i18n::t("settings.security.mode.local_encrypted")
            }
            StorageMode::ConsentPending => {
                wormhole_i18n::t("settings.security.mode.consent_pending")
            }
            StorageMode::Declined => wormhole_i18n::t("settings.security.mode.declined"),
        }
    }

    fn action_button(
        &self,
        label: &str,
        action: SecurityAction,
        automation_id: &str,
        primary: bool,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(if primary {
                        theme::canvas()
                    } else {
                        theme::text()
                    })
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(8.0)
        .with_padding_bottom(8.0)
        .with_background(if primary {
            theme::accent()
        } else {
            theme::bg()
        })
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
    }
}

impl Entity for SecurityView {
    type Event = ();
}

impl TypedActionView for SecurityView {
    type Action = SecurityAction;

    fn handle_action(&mut self, action: &SecurityAction, ctx: &mut ViewContext<Self>) {
        match action {
            SecurityAction::Refresh => {
                self.refresh();
                ctx.notify();
            }
            SecurityAction::RetryProbe => {
                self.status = keyring_consent_retry_probe();
                self.message = wormhole_i18n::t("settings.security.probe_done");
                self.tone = if self.status.available {
                    StatusTone::Success
                } else {
                    StatusTone::Warn
                };
                ctx.notify();
            }
            SecurityAction::ConsentLocalEncrypted => {
                match keyring_consent_decide(&self.core.data_dir(), "local_encrypted") {
                    Ok(_) => {
                        self.refresh();
                        self.message = wormhole_i18n::t("settings.security.local_ok");
                        self.tone = StatusTone::Success;
                    }
                    Err(err) => {
                        self.message = err;
                        self.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            }
            SecurityAction::ConsentDeclined => {
                match keyring_consent_decide(&self.core.data_dir(), "declined") {
                    Ok(_) => {
                        self.refresh();
                        self.message = wormhole_i18n::t("settings.security.declined_ok");
                        self.tone = StatusTone::Warn;
                    }
                    Err(err) => {
                        self.message = err;
                        self.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            }
        }
    }
}

impl View for SecurityView {
    fn ui_name() -> &'static str {
        "SecurityView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        // Settings content lives inside a vertical Scrollable (infinite max height).
        // Do not use MainAxisSize::Max here — WarpUI panics on expand-in-infinite.
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        col.add_child(section_title(
            wormhole_i18n::t("settings.security.title"),
            self.font,
        ));
        col.add_child(
            Container::new(section_hint(
                wormhole_i18n::t("settings.security.hint"),
                self.font,
            ))
            .with_margin_top(SECTION_GAP)
            .finish(),
        );

        let mut card = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        card.add_child(
            ui_text::body(
                format!(
                    "{}: {}",
                    wormhole_i18n::t("settings.security.mode_label"),
                    Self::mode_label(self.status.active_mode)
                ),
                self.font,
            )
            .with_color(theme::text())
            .finish(),
        );
        card.add_child(
            Container::new(
                ui_text::body(
                    format!(
                        "{}: {} ({})",
                        wormhole_i18n::t("settings.security.backend"),
                        self.status.backend_name,
                        if self.status.available {
                            wormhole_i18n::t("settings.security.available")
                        } else {
                            wormhole_i18n::t("settings.security.unavailable")
                        }
                    ),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
        if let Some(reason) = &self.status.failure_reason {
            card.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{}: {reason}",
                            wormhole_i18n::t("settings.security.failure_reason")
                        ),
                        self.font,
                    )
                    .with_color(theme::danger())
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(section_card(card.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut actions = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
        actions.add_child(self.action_button(
            &wormhole_i18n::t("settings.security.retry"),
            SecurityAction::RetryProbe,
            "settings:security_retry",
            true,
        ));
        if !self.status.available {
            actions.add_child(
                Container::new(self.action_button(
                    &wormhole_i18n::t("settings.security.use_local"),
                    SecurityAction::ConsentLocalEncrypted,
                    "settings:security_local",
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
            actions.add_child(
                Container::new(self.action_button(
                    &wormhole_i18n::t("settings.security.decline"),
                    SecurityAction::ConsentDeclined,
                    "settings:security_decline",
                    false,
                ))
                .with_margin_left(8.0)
                .finish(),
            );
        }
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.security.refresh"),
                SecurityAction::Refresh,
                "settings:security_refresh",
                false,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        if !self.message.is_empty() {
            col.add_child(
                Container::new(status_line(self.message.clone(), self.font, self.tone))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            );
        }

        EventHandler::new(
            Container::new(col.finish())
                .with_uniform_padding(16.)
                .finish(),
        )
        .with_automation_id("settings:security_panel")
        .with_automation_label(wormhole_i18n::t("settings.security.title"))
        // Register as an automation target (hit-testable) for sim-use.
        .on_left_mouse_down(|_ctx, _, _| DispatchEventResult::PropagateToParent)
        .finish()
    }
}
