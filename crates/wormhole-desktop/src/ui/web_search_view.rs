//! Settings → Web Search: BYOK Brave / Exa for Agent `web_search`.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::web_commands::{
    web_search_config, web_search_configure, WebSearchConfigDto, WebSearchConfigureParams,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineChoice {
    Disabled,
    Brave,
    Exa,
}

impl EngineChoice {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Brave => "brave",
            Self::Exa => "exa",
        }
    }

    fn from_dto(engine: &str) -> Self {
        match engine.trim().to_ascii_lowercase().as_str() {
            "brave" => Self::Brave,
            "exa" => Self::Exa,
            _ => Self::Disabled,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WebSearchAction {
    Refresh,
    Save,
    SelectEngine(EngineChoice),
    FocusBraveKey,
    FocusExaKey,
    BraveKeyEdit(TextFieldEditAction),
    ExaKeyEdit(TextFieldEditAction),
}

pub struct WebSearchView {
    core: CoreHandle,
    font: FamilyId,
    engine: EngineChoice,
    brave_configured: bool,
    exa_configured: bool,
    config_path: String,
    brave_key_draft: String,
    exa_key_draft: String,
    brave_field: TextFieldState,
    exa_field: TextFieldState,
    brave_focused: bool,
    exa_focused: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl WebSearchView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            engine: EngineChoice::Disabled,
            brave_configured: false,
            exa_configured: false,
            config_path: String::new(),
            brave_key_draft: String::new(),
            exa_key_draft: String::new(),
            brave_field: TextFieldState::new(),
            exa_field: TextFieldState::new(),
            brave_focused: false,
            exa_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn apply_dto(&mut self, dto: WebSearchConfigDto) {
        self.engine = EngineChoice::from_dto(&dto.engine);
        self.brave_configured = dto.brave_configured;
        self.exa_configured = dto.exa_configured;
        self.config_path = dto.config_path;
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.web_search.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                web_search_config(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_dto(dto);
                        view.message = wormhole_i18n::t("settings.web_search.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.web_search.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn save(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.web_search.saving");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let params = WebSearchConfigureParams {
            engine: self.engine.as_str().to_string(),
            brave_api_key: {
                let t = self.brave_key_draft.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            },
            exa_api_key: {
                let t = self.exa_key_draft.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            },
            allowed_domains: None,
            max_results: None,
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                web_search_configure(&state, params).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_dto(dto);
                        view.brave_key_draft.clear();
                        view.exa_key_draft.clear();
                        view.message = wormhole_i18n::t("settings.web_search.saved");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.web_search.save_failed")
                        );
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
        action: WebSearchAction,
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

    fn engine_chip(&self, choice: EngineChoice, label: &str, automation_id: &str) -> Box<dyn Element> {
        self.action_chip(
            label,
            WebSearchAction::SelectEngine(choice),
            self.engine == choice,
            automation_id,
            self.busy,
        )
    }

    fn key_field(
        &self,
        label: &str,
        configured: bool,
        draft: &str,
        field: &TextFieldState,
        focused: bool,
        focus_action: WebSearchAction,
        edit_to_action: fn(TextFieldEditAction) -> WebSearchAction,
        automation_id: &str,
        placeholder: &str,
    ) -> Box<dyn Element> {
        let status = if configured {
            wormhole_i18n::t("settings.web_search.key_set")
        } else {
            wormhole_i18n::t("settings.web_search.key_missing")
        };
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
                Flex::row()
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        ui_text::body(label.to_string(), self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::body(status, self.font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .with_margin_left(8.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_child(
                EventHandler::new(
                    Container::new(input)
                        .with_uniform_padding(10.0)
                        .with_margin_top(8.0)
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
}

impl Entity for WebSearchView {
    type Event = ();
}

impl TypedActionView for WebSearchView {
    type Action = WebSearchAction;

    fn handle_action(&mut self, action: &WebSearchAction, ctx: &mut ViewContext<Self>) {
        match action {
            WebSearchAction::Refresh => self.refresh(ctx),
            WebSearchAction::Save => self.save(ctx),
            WebSearchAction::SelectEngine(choice) => {
                self.engine = *choice;
                ctx.notify();
            }
            WebSearchAction::FocusBraveKey => {
                self.brave_focused = true;
                self.exa_focused = false;
                ctx.notify();
            }
            WebSearchAction::FocusExaKey => {
                self.exa_focused = true;
                self.brave_focused = false;
                ctx.notify();
            }
            WebSearchAction::BraveKeyEdit(edit) => {
                self.brave_field.apply(&mut self.brave_key_draft, edit);
                ctx.notify();
            }
            WebSearchAction::ExaKeyEdit(edit) => {
                self.exa_field.apply(&mut self.exa_key_draft, edit);
                ctx.notify();
            }
        }
    }
}

impl View for WebSearchView {
    fn ui_name() -> &'static str {
        "WebSearchView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.web_search.title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.web_search.hint"),
            self.font,
        ));
        col.add_child(
            EventHandler::new(status_line(self.message.clone(), self.font, self.tone))
                .with_automation_id("settings:web_search_status")
                .with_automation_label(wormhole_i18n::t("settings.web_search.status_label"))
                .finish(),
        );

        let mut engine_row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        engine_row.add_child(self.engine_chip(
            EngineChoice::Disabled,
            &wormhole_i18n::t("settings.web_search.engine_disabled"),
            "settings:web_search_engine_disabled",
        ));
        engine_row.add_child(
            Container::new(self.engine_chip(
                EngineChoice::Brave,
                &wormhole_i18n::t("settings.web_search.engine_brave"),
                "settings:web_search_engine_brave",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        engine_row.add_child(
            Container::new(self.engine_chip(
                EngineChoice::Exa,
                &wormhole_i18n::t("settings.web_search.engine_exa"),
                "settings:web_search_engine_exa",
            ))
            .with_margin_left(8.0)
            .finish(),
        );

        let mut engine_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        engine_col.add_child(section_title(
            wormhole_i18n::t("settings.web_search.engine_title"),
            self.font,
        ));
        engine_col.add_child(
            Container::new(engine_row.finish())
                .with_margin_top(8.0)
                .finish(),
        );
        col.add_child(
            Container::new(section_card(engine_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut keys_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        keys_col.add_child(section_title(
            wormhole_i18n::t("settings.web_search.keys_title"),
            self.font,
        ));
        keys_col.add_child(
            Container::new(self.key_field(
                &wormhole_i18n::t("settings.web_search.brave_key"),
                self.brave_configured,
                &self.brave_key_draft,
                &self.brave_field,
                self.brave_focused,
                WebSearchAction::FocusBraveKey,
                WebSearchAction::BraveKeyEdit,
                "settings:web_search_brave_key",
                &wormhole_i18n::t("settings.web_search.key_placeholder"),
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        keys_col.add_child(
            Container::new(self.key_field(
                &wormhole_i18n::t("settings.web_search.exa_key"),
                self.exa_configured,
                &self.exa_key_draft,
                &self.exa_field,
                self.exa_focused,
                WebSearchAction::FocusExaKey,
                WebSearchAction::ExaKeyEdit,
                "settings:web_search_exa_key",
                &wormhole_i18n::t("settings.web_search.key_placeholder"),
            ))
            .with_margin_top(12.0)
            .finish(),
        );
        col.add_child(
            Container::new(section_card(keys_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        if !self.config_path.is_empty() {
            col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{} {}",
                            wormhole_i18n::t("settings.web_search.config_path"),
                            self.config_path
                        ),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(SECTION_GAP)
                .finish(),
            );
        }

        let mut actions = Flex::row().with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.action_chip(
            &wormhole_i18n::t("settings.web_search.refresh"),
            WebSearchAction::Refresh,
            false,
            "settings:web_search_refresh",
            self.busy,
        ));
        actions.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.web_search.save"),
                WebSearchAction::Save,
                true,
                "settings:web_search_save",
                self.busy,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        Container::new(col.finish()).finish()
    }
}
