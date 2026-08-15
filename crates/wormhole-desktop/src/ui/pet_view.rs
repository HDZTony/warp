//! Settings → Desktop pet: overlay, wake word, and meeting camera.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::{
    agent_pet_set_enabled, agent_pet_set_vcam_enabled, agent_pet_status, voice_wake_set_enabled,
    voice_wake_trigger_manual, AgentPetStatusDto,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{
    section_card, section_hint, section_title, status_line, StatusTone, SECTION_GAP,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PetToggle {
    Enabled,
    VoiceWake,
    Vcam,
}

#[derive(Debug, Clone)]
pub enum PetAction {
    Refresh,
    Toggle(PetToggle),
    ManualWake,
}

pub struct PetView {
    core: CoreHandle,
    font: FamilyId,
    status: Option<AgentPetStatusDto>,
    message: String,
    tone: StatusTone,
    busy: bool,
}

fn status_copy(dto: &AgentPetStatusDto) -> (String, StatusTone) {
    if !dto.enabled {
        return (
            wormhole_i18n::t("settings.pet.status_off"),
            StatusTone::Muted,
        );
    }
    if dto.running {
        return (
            wormhole_i18n::t("settings.pet.running"),
            StatusTone::Success,
        );
    }
    if !dto.binary_available {
        return (
            wormhole_i18n::t("settings.pet.binary_missing"),
            StatusTone::Warn,
        );
    }
    (
        wormhole_i18n::t("settings.pet.status_on_not_running"),
        StatusTone::Warn,
    )
}

impl PetView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            status: None,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.pet.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move { agent_pet_status(core.app_state()).await },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        let (copy, tone) = status_copy(&dto);
                        view.status = Some(dto);
                        view.message = copy;
                        view.tone = tone;
                    }
                    Err(err) => {
                        view.message = format!(
                            "{}: {err}",
                            wormhole_i18n::t("settings.pet.load_failed")
                        );
                        view.tone = StatusTone::Warn;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn toggle(&mut self, which: PetToggle, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let Some(current) = &self.status else {
            return;
        };
        let enabled = match which {
            PetToggle::Enabled => !current.enabled,
            PetToggle::VoiceWake => !current.voice_wake_enabled,
            PetToggle::Vcam => !current.vcam_enabled,
        };
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                match which {
                    PetToggle::Enabled => {
                        agent_pet_set_enabled(core.app_state(), enabled).await
                    }
                    PetToggle::VoiceWake => {
                        let _ = voice_wake_set_enabled(core.app_state(), enabled).await?;
                        agent_pet_status(core.app_state()).await
                    }
                    PetToggle::Vcam => {
                        agent_pet_set_vcam_enabled(core.app_state(), enabled).await
                    }
                }
            },
            move |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        let (copy, tone) = status_copy(&dto);
                        view.status = Some(dto);
                        view.message = copy;
                        view.tone = tone;
                    }
                    Err(err) => {
                        view.message = format!(
                            "{}: {err}",
                            wormhole_i18n::t("settings.pet.toggle_failed")
                        );
                        view.tone = StatusTone::Warn;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn manual_wake(&mut self, ctx: &mut ViewContext<Self>) {
        match voice_wake_trigger_manual() {
            Ok(()) => {
                self.message = wormhole_i18n::t("settings.pet.manual_wake_ok");
                self.tone = StatusTone::Success;
            }
            Err(err) => {
                self.message = format!(
                    "{}: {err}",
                    wormhole_i18n::t("settings.pet.manual_wake_failed")
                );
                self.tone = StatusTone::Warn;
            }
        }
        ctx.notify();
    }

    fn toggle_row(
        &self,
        title: String,
        hint: String,
        on: bool,
        which: PetToggle,
        automation_id: &'static str,
    ) -> Box<dyn Element> {
        let on_label = if on {
            wormhole_i18n::t("settings.pet.on")
        } else {
            wormhole_i18n::t("settings.pet.off")
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
                            ui_text::body(hint, self.font)
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
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(PetAction::Toggle(which));
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn action_button(
        &self,
        label: String,
        action: PetAction,
        automation_id: &'static str,
    ) -> Box<dyn Element> {
        EventHandler::new(
            Container::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(8.0)
            .with_padding_bottom(8.0)
            .with_background(theme::accent_cool_bg(32))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_automation_label(label)
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    fn switches_card(&self, dto: &AgentPetStatusDto) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.pet.enabled"),
            self.font,
        ));
        col.add_child(
            Container::new(self.toggle_row(
                wormhole_i18n::t("settings.pet.enabled"),
                wormhole_i18n::t("settings.pet.enabled_hint"),
                dto.enabled,
                PetToggle::Enabled,
                "settings:pet_enabled",
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(self.toggle_row(
                wormhole_i18n::t("settings.pet.voice_wake"),
                wormhole_i18n::t("settings.pet.voice_wake_hint"),
                dto.voice_wake_enabled,
                PetToggle::VoiceWake,
                "settings:pet_voice_wake",
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        if dto.voice_wake_enabled {
            col.add_child(
                Container::new(self.action_button(
                    wormhole_i18n::t("settings.pet.manual_wake"),
                    PetAction::ManualWake,
                    "settings:pet_voice_wake_manual",
                ))
                .with_margin_top(8.0)
                .finish(),
            );
            col.add_child(
                Container::new(
                    ui_text::body(
                        wormhole_i18n::t("settings.pet.manual_wake_hint"),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(4.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(self.toggle_row(
                wormhole_i18n::t("settings.pet.vcam"),
                wormhole_i18n::t("settings.pet.vcam_hint"),
                dto.vcam_enabled,
                PetToggle::Vcam,
                "settings:pet_vcam",
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        section_card(col.finish())
    }
}

impl Entity for PetView {
    type Event = ();
}

impl View for PetView {
    fn ui_name() -> &'static str {
        "PetView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.pet.hint"),
            self.font,
        ));
        col.add_child(
            Container::new(status_line(self.message.clone(), self.font, self.tone))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );
        if let Some(dto) = &self.status {
            col.add_child(
                Container::new(self.switches_card(dto))
                    .with_margin_top(SECTION_GAP)
                    .finish(),
            );
        }
        col.finish()
    }
}

impl TypedActionView for PetView {
    type Action = PetAction;

    fn handle_action(&mut self, action: &PetAction, ctx: &mut ViewContext<Self>) {
        match action {
            PetAction::Refresh => self.refresh(ctx),
            PetAction::Toggle(which) => self.toggle(*which, ctx),
            PetAction::ManualWake => self.manual_wake(ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::status_copy;
    use wormhole_desktop_core::AgentPetStatusDto;

    fn dto(enabled: bool, running: bool, binary_available: bool) -> AgentPetStatusDto {
        AgentPetStatusDto {
            enabled,
            binary_available,
            binary_path: None,
            running,
            pid: None,
            vcam_enabled: false,
            vcam_dir: None,
            voice_wake_enabled: false,
            attention: None,
        }
    }

    #[test]
    fn status_copy_covers_off_running_and_missing_binary() {
        wormhole_i18n::set_locale("zh-CN");
        assert_eq!(
            status_copy(&dto(false, false, true)).0,
            wormhole_i18n::t("settings.pet.status_off")
        );
        assert_eq!(
            status_copy(&dto(true, true, true)).0,
            wormhole_i18n::t("settings.pet.running")
        );
        assert_eq!(
            status_copy(&dto(true, false, false)).0,
            wormhole_i18n::t("settings.pet.binary_missing")
        );
        assert_eq!(
            status_copy(&dto(true, false, true)).0,
            wormhole_i18n::t("settings.pet.status_on_not_running")
        );
    }
}
