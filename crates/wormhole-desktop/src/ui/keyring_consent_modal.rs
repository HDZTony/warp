//! Full-window consent when OS keyring is unavailable.

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{status_line, StatusTone};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::security::{
    keyring_consent_decide, keyring_consent_retry_probe, keyring_consent_status, KeyringStatus,
    StorageMode,
};

#[derive(Debug, Clone)]
pub enum KeyringConsentAction {
    Refresh,
    UseLocalEncrypted,
    RetryOsKeyring,
    Decline,
    ToggleDetails,
}

#[derive(Debug, Clone)]
pub enum KeyringConsentEvent {
    OpenChanged { open: bool },
    Decided { mode: StorageMode },
}

pub struct KeyringConsentModalView {
    core: CoreHandle,
    font: FamilyId,
    open: bool,
    status: KeyringStatus,
    details_open: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl KeyringConsentModalView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let status = keyring_consent_status();
        let open = status.consent_required;
        Self {
            core,
            font: crate::ui::fonts::load_ui_font(ctx),
            open,
            status,
            details_open: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn refresh_visibility(&mut self, ctx: &mut ViewContext<Self>) {
        self.status = keyring_consent_status();
        let open = self.status.consent_required;
        if open != self.open {
            self.open = open;
            ctx.emit(KeyringConsentEvent::OpenChanged { open });
        }
        ctx.notify();
    }

    fn apply_decision(&mut self, choice: &str, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        match keyring_consent_decide(&self.core.data_dir(), choice) {
            Ok(_) => {
                self.status = keyring_consent_status();
                self.open = self.status.consent_required;
                self.message = wormhole_i18n::t("keyring.consent.saved");
                self.tone = StatusTone::Success;
                ctx.emit(KeyringConsentEvent::Decided {
                    mode: self.status.active_mode,
                });
                ctx.emit(KeyringConsentEvent::OpenChanged { open: self.open });
            }
            Err(err) => {
                self.message = err;
                self.tone = StatusTone::Danger;
            }
        }
        self.busy = false;
        ctx.notify();
    }

    fn action_button(
        &self,
        label: &str,
        action: KeyringConsentAction,
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
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .with_background(if primary {
            theme::accent()
        } else {
            theme::bg()
        })
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
    }

    fn dialog(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

        col.add_child(
            ui_text::body(wormhole_i18n::t("keyring.consent.title"), self.font)
                .with_color(theme::text())
                .finish(),
        );
        col.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("keyring.consent.body"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
        if let Some(reason) = &self.status.failure_reason {
            col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{}: {reason}",
                            wormhole_i18n::t("keyring.consent.failure")
                        ),
                        self.font,
                    )
                    .with_color(theme::danger())
                    .finish(),
                )
                .with_margin_top(10.0)
                .finish(),
            );
        }

        col.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("keyring.consent.use_local"),
                KeyringConsentAction::UseLocalEncrypted,
                "keyring:consent_local",
                true,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
        col.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("keyring.consent.retry"),
                KeyringConsentAction::RetryOsKeyring,
                "keyring:consent_retry",
                false,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("keyring.consent.skip"),
                KeyringConsentAction::Decline,
                "keyring:consent_skip",
                false,
            ))
            .with_margin_top(8.0)
            .finish(),
        );

        let details_label = if self.details_open {
            wormhole_i18n::t("keyring.consent.hide_details")
        } else {
            wormhole_i18n::t("keyring.consent.show_details")
        };
        col.add_child(
            Container::new(self.action_button(
                &details_label,
                KeyringConsentAction::ToggleDetails,
                "keyring:consent_details",
                false,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if self.details_open {
            col.add_child(
                Container::new(
                    ui_text::body(wormhole_i18n::t("keyring.consent.details"), self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }

        if !self.message.is_empty() {
            col.add_child(
                Container::new(status_line(self.message.clone(), self.font, self.tone))
                    .with_margin_top(10.0)
                    .finish(),
            );
        }

        let card = EventHandler::new(
            Container::new(col.finish())
                .with_background(theme::bg())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
                .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                .with_uniform_padding(20.)
                .finish(),
        )
        .with_automation_id("keyring:consent_dialog")
        .with_automation_label(wormhole_i18n::t("keyring.consent.title"))
        .finish();

        ConstrainedBox::new(Align::new(card).finish())
            .with_max_width(480.)
            .finish()
    }
}

impl Entity for KeyringConsentModalView {
    type Event = KeyringConsentEvent;
}

impl TypedActionView for KeyringConsentModalView {
    type Action = KeyringConsentAction;

    fn handle_action(&mut self, action: &KeyringConsentAction, ctx: &mut ViewContext<Self>) {
        match action {
            KeyringConsentAction::Refresh => self.refresh_visibility(ctx),
            KeyringConsentAction::UseLocalEncrypted => {
                self.apply_decision("local_encrypted", ctx);
            }
            KeyringConsentAction::RetryOsKeyring => {
                self.status = keyring_consent_retry_probe(&self.core.data_dir());
                self.open = self.status.consent_required;
                self.message = if self.status.available {
                    wormhole_i18n::t("keyring.consent.retry_ok")
                } else {
                    wormhole_i18n::t("keyring.consent.retry_fail")
                };
                self.tone = if self.status.available {
                    StatusTone::Success
                } else {
                    StatusTone::Warn
                };
                ctx.emit(KeyringConsentEvent::OpenChanged { open: self.open });
                if self.status.available {
                    ctx.emit(KeyringConsentEvent::Decided {
                        mode: self.status.active_mode,
                    });
                }
                ctx.notify();
            }
            KeyringConsentAction::Decline => self.apply_decision("declined", ctx),
            KeyringConsentAction::ToggleDetails => {
                self.details_open = !self.details_open;
                ctx.notify();
            }
        }
    }
}

impl View for KeyringConsentModalView {
    fn ui_name() -> &'static str {
        "KeyringConsentModalView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        if !self.open {
            return Flex::column().finish();
        }

        let dialog = self.dialog();
        let mut center = Flex::column()
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        center.add_child(dialog);

        let scrim = Container::new(center.finish())
            .with_background(ColorU::new(8, 7, 11, 200))
            .finish();

        Expanded::new(
            1.0,
            EventHandler::new(scrim)
                .with_automation_id("keyring:consent_scrim")
                .with_automation_label(wormhole_i18n::t("keyring.consent.title"))
                .on_left_mouse_down(|_ctx, _, _| DispatchEventResult::StopPropagation)
                .finish(),
        )
        .finish()
    }
}
