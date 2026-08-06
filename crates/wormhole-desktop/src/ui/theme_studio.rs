//! Settings → Theme Studio panel (OpenHuman Theme Studio parity).

use std::path::Path;

use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::clipboard::{read_clipboard_text, write_clipboard_text};
use crate::ui::desktop_prefs::{self, DesktopUiPrefs};
use crate::ui::panel_primitives::{section_hint, status_line, StatusTone};
use crate::ui::text_field_input::{
    render_field_with_caret, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme::{
    self, active_contrast_warning, color_u_to_hex, delete_custom_theme, export_theme_json,
    family_gallery, import_theme_json, reset_active_theme, resolve_theme, select_family,
    selected_family_id, set_backdrop, set_color_token, set_font_role, set_variant,
    upsert_custom_theme, BackdropKind, FontRole, ThemeSelection, ThemeVariant, COLOR_GROUPS,
    FONT_CHOICES,
};
use crate::ui_text;

/// Transient UI state for Theme Studio editors.
#[derive(Clone)]
pub struct ThemeStudioState {
    pub token_draft: String,
    pub token_key: Option<String>,
    pub token_field: TextFieldState,
    pub token_focused: bool,
    pub import_draft: String,
    pub import_field: TextFieldState,
    pub import_focused: bool,
    pub message: String,
    pub tone: StatusTone,
    pub export_preview: String,
}

impl Default for ThemeStudioState {
    fn default() -> Self {
        Self {
            token_draft: String::new(),
            token_key: None,
            token_field: TextFieldState::new(),
            token_focused: false,
            import_draft: String::new(),
            import_field: TextFieldState::new(),
            import_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            export_preview: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ThemeStudioAction {
    SelectFamily(String),
    SetVariant(ThemeVariant),
    BeginEditToken(String),
    FocusTokenField,
    TokenFieldEdit(TextFieldEditAction),
    ApplyTokenHex,
    SetFontRole { role: String, choice: String },
    SetBackdropKind(BackdropKind),
    ToggleBackdropDots,
    ResetActive,
    DeleteActive,
    ExportActive,
    FocusImport,
    ImportFieldEdit(TextFieldEditAction),
    ImportDraft,
    ImportClipboard,
    NewBlankCustom,
}

fn system_is_dark_from_app(app: &warpui::AppContext) -> bool {
    matches!(app.system_theme(), warpui::platform::SystemTheme::Dark)
}

fn load_selection(data_dir: &Path) -> ThemeSelection {
    desktop_prefs::load(data_dir).theme_selection()
}

fn persist_and_apply(
    data_dir: &Path,
    selection: &ThemeSelection,
    system_is_dark: bool,
) -> Result<(), String> {
    desktop_prefs::set_theme_selection(data_dir, selection, system_is_dark).map(|_| ())
}

pub fn handle_action(
    state: &mut ThemeStudioState,
    data_dir: &Path,
    action: ThemeStudioAction,
    system_is_dark: bool,
) {
    let mut selection = load_selection(data_dir);
    let result = match action {
        ThemeStudioAction::SelectFamily(id) => {
            select_family(&mut selection, &id).and_then(|_| {
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t_args(
                    "settings.theme.msg.selected",
                    &[("name", &id)],
                ))
            })
        }
        ThemeStudioAction::SetVariant(variant) => {
            set_variant(&mut selection, variant);
            persist_and_apply(data_dir, &selection, system_is_dark).map(|_| {
                wormhole_i18n::t_args(
                    "settings.theme.msg.variant",
                    &[("variant", variant.as_str())],
                )
            })
        }
        ThemeStudioAction::BeginEditToken(key) => {
            // Start the editor empty so typing replaces rather than appending.
            // Current value remains visible on the token row.
            state.token_key = Some(key);
            state.token_draft.clear();
            state.token_field = TextFieldState::new();
            state.token_focused = true;
            state.import_focused = false;
            state.message.clear();
            return;
        }
        ThemeStudioAction::FocusTokenField => {
            state.token_focused = true;
            state.import_focused = false;
            return;
        }
        ThemeStudioAction::TokenFieldEdit(edit) => {
            state.token_field.apply(&mut state.token_draft, &edit);
            return;
        }
        ThemeStudioAction::ApplyTokenHex => {
            let Some(key) = state.token_key.clone() else {
                state.message = wormhole_i18n::t("settings.theme.msg.no_token");
                state.tone = StatusTone::Warn;
                return;
            };
            set_color_token(&mut selection, system_is_dark, &key, &state.token_draft).and_then(
                |_| {
                    persist_and_apply(data_dir, &selection, system_is_dark)?;
                    Ok(wormhole_i18n::t_args(
                        "settings.theme.msg.token_set",
                        &[("key", &key)],
                    ))
                },
            )
        }
        ThemeStudioAction::SetFontRole { role, choice } => {
            set_font_role(&mut selection, system_is_dark, &role, &choice).and_then(|_| {
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t("settings.theme.msg.font_set"))
            })
        }
        ThemeStudioAction::SetBackdropKind(kind) => {
            let mut backdrop = resolve_theme(&selection, system_is_dark).backdrop;
            backdrop.kind = kind;
            set_backdrop(&mut selection, system_is_dark, backdrop);
            persist_and_apply(data_dir, &selection, system_is_dark)
                .map(|_| wormhole_i18n::t("settings.theme.msg.backdrop_set"))
        }
        ThemeStudioAction::ToggleBackdropDots => {
            let mut backdrop = resolve_theme(&selection, system_is_dark).backdrop;
            backdrop.dots = !backdrop.dots;
            set_backdrop(&mut selection, system_is_dark, backdrop);
            persist_and_apply(data_dir, &selection, system_is_dark)
                .map(|_| wormhole_i18n::t("settings.theme.msg.backdrop_set"))
        }
        ThemeStudioAction::ResetActive => reset_active_theme(&mut selection, system_is_dark)
            .and_then(|_| {
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t("settings.theme.msg.reset"))
            }),
        ThemeStudioAction::DeleteActive => {
            let id = selection.active_theme_id.clone();
            delete_custom_theme(&mut selection, &id).and_then(|_| {
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t("settings.theme.msg.deleted"))
            })
        }
        ThemeStudioAction::ExportActive => {
            let theme = resolve_theme(&selection, system_is_dark);
            match export_theme_json(&theme) {
                Ok(json) => {
                    state.export_preview = json.clone();
                    match write_clipboard_text(&json) {
                        Ok(()) => {
                            state.message = wormhole_i18n::t("settings.theme.msg.exported");
                            state.tone = StatusTone::Success;
                        }
                        Err(err) => {
                            state.message = err;
                            state.tone = StatusTone::Warn;
                        }
                    }
                }
                Err(err) => {
                    state.message = err;
                    state.tone = StatusTone::Danger;
                }
            }
            return;
        }
        ThemeStudioAction::FocusImport => {
            state.import_focused = true;
            state.token_focused = false;
            return;
        }
        ThemeStudioAction::ImportFieldEdit(edit) => {
            state.import_field.apply(&mut state.import_draft, &edit);
            return;
        }
        ThemeStudioAction::ImportDraft => {
            import_theme_json(&state.import_draft).and_then(|theme| {
                let name = theme.name.clone();
                let id = theme.id.clone();
                upsert_custom_theme(&mut selection, theme);
                selection.active_theme_id = id;
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t_args(
                    "settings.theme.msg.imported",
                    &[("name", &name)],
                ))
            })
        }
        ThemeStudioAction::ImportClipboard => {
            let Some(raw) = read_clipboard_text() else {
                state.message = wormhole_i18n::t("settings.theme.msg.clipboard_empty");
                state.tone = StatusTone::Warn;
                return;
            };
            state.import_draft = raw.clone();
            import_theme_json(&raw).and_then(|theme| {
                let name = theme.name.clone();
                let id = theme.id.clone();
                upsert_custom_theme(&mut selection, theme);
                selection.active_theme_id = id;
                persist_and_apply(data_dir, &selection, system_is_dark)?;
                Ok(wormhole_i18n::t_args(
                    "settings.theme.msg.imported",
                    &[("name", &name)],
                ))
            })
        }
        ThemeStudioAction::NewBlankCustom => {
            let id = format!(
                "custom-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            );
            let mut blank = resolve_theme(&selection, system_is_dark);
            let based = if blank.is_dark { "dark" } else { "light" };
            blank.id = id.clone();
            blank.name = wormhole_i18n::t("settings.theme.custom_name");
            blank.built_in = false;
            blank.based_on = Some(based.into());
            upsert_custom_theme(&mut selection, blank);
            selection.active_theme_id = id;
            persist_and_apply(data_dir, &selection, system_is_dark)
                .map(|_| wormhole_i18n::t("settings.theme.msg.created"))
        }
    };

    match result {
        Ok(msg) => {
            state.message = msg;
            state.tone = StatusTone::Success;
        }
        Err(err) => {
            state.message = err;
            state.tone = StatusTone::Danger;
        }
    }
}

fn dispatch_theme(action: ThemeStudioAction) -> crate::ui::settings_view::SettingsAction {
    crate::ui::settings_view::SettingsAction::ThemeStudio(action)
}

fn chip_button(
    label: String,
    selected: bool,
    action: ThemeStudioAction,
    font: FamilyId,
    automation_id: String,
) -> Box<dyn Element> {
    let text_color = if selected {
        theme::text()
    } else {
        theme::muted()
    };
    Container::new(
        EventHandler::new(
            ui_text::body(label.clone(), font)
                .with_color(text_color)
                .finish(),
        )
        .with_automation_label(label)
        .with_automation_id(automation_id)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(dispatch_theme(action.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_padding_left(10.0)
    .with_padding_right(10.0)
    .with_padding_top(6.0)
    .with_padding_bottom(6.0)
    .with_background(if selected {
        theme::accent_cool_bg(40)
    } else {
        theme::panel()
    })
    .with_border(
        Border::all(1.0).with_border_fill(if selected {
            theme::accent_cool()
        } else {
            theme::border()
        }),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
    .finish()
}

fn swatch(color: ColorU) -> Box<dyn Element> {
    ConstrainedBox::new(
        Container::new(warpui::elements::Empty::new().finish())
            .with_background(color)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .finish(),
    )
    .with_width(22.0)
    .with_height(22.0)
    .finish()
}

fn text_field(
    draft: &str,
    field: &TextFieldState,
    focused: bool,
    placeholder: &str,
    font: FamilyId,
    automation_id: &str,
    automation_label: &str,
    on_focus: ThemeStudioAction,
    map_edit: fn(TextFieldEditAction) -> ThemeStudioAction,
) -> Box<dyn Element> {
    let marked = field.marked_text.clone();
    TextFieldInput::builder(
        EventHandler::new(
            Container::new(render_field_with_caret(
                draft,
                &marked,
                placeholder,
                font,
                focused,
                false,
                true,
                field.cursor,
            ))
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(if focused {
                theme::accent_cool()
            } else {
                theme::border()
            }))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
            .finish(),
        )
        .with_automation_label(automation_label.to_string())
        .with_automation_id(automation_id.to_string())
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(dispatch_theme(on_focus.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish(),
        move |ctx, action| {
            ctx.dispatch_typed_action(dispatch_theme(map_edit(action)));
        },
    )
    .focused(focused)
    .disabled(false)
    .ime_preedit(!marked.is_empty())
    .finish()
}

/// Build the Theme Studio settings body.
pub fn render_panel(
    prefs: &DesktopUiPrefs,
    studio: &ThemeStudioState,
    font: FamilyId,
    system_is_dark: bool,
) -> Box<dyn Element> {
    let selection = prefs.theme_selection();
    let active = resolve_theme(&selection, system_is_dark);
    let palette = theme::runtime::palette_from_theme(&active);
    let family_id = selected_family_id(&selection);

    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(section_hint(
        wormhole_i18n::t("settings.theme.hint"),
        font,
    ));

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.variant_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(14.0)
        .finish(),
    );
    let mut variant_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for (variant, label_key) in [
        (ThemeVariant::Light, "settings.theme.variant.light"),
        (ThemeVariant::Dark, "settings.theme.variant.dark"),
        (ThemeVariant::System, "settings.theme.variant.system"),
    ] {
        let selected = selection.theme_variant == variant;
        variant_row.add_child(
            Container::new(chip_button(
                wormhole_i18n::t(label_key),
                selected,
                ThemeStudioAction::SetVariant(variant),
                font,
                format!("settings:theme:variant:{}", variant.as_str()),
            ))
            .with_margin_right(8.0)
            .with_margin_top(8.0)
            .finish(),
        );
    }
    col.add_child(variant_row.finish());

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.family_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(16.0)
        .finish(),
    );
    let mut gallery = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for family in family_gallery() {
        let selected = family_id == family.id;
        gallery.add_child(
            Container::new(chip_button(
                family.name.to_string(),
                selected,
                ThemeStudioAction::SelectFamily(family.id.to_string()),
                font,
                format!("settings:theme:family:{}", family.id),
            ))
            .with_margin_right(8.0)
            .with_margin_top(8.0)
            .finish(),
        );
    }
    for custom in &selection.custom_themes {
        let selected = selection.active_theme_id == custom.id;
        gallery.add_child(
            Container::new(chip_button(
                custom.name.clone(),
                selected,
                ThemeStudioAction::SelectFamily(custom.id.clone()),
                font,
                format!("settings:theme:family:{}", custom.id),
            ))
            .with_margin_right(8.0)
            .with_margin_top(8.0)
            .finish(),
        );
    }
    col.add_child(gallery.finish());

    if active.built_in {
        col.add_child(
            Container::new(
                ui_text::mono(wormhole_i18n::t("settings.theme.autofork_hint"), font)
                    .with_color(theme::placeholder())
                    .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
    }

    if active_contrast_warning(&palette) {
        col.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("settings.theme.contrast_warn"), font)
                    .with_color(theme::warn())
                    .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
    }

    for group in COLOR_GROUPS {
        col.add_child(
            Container::new(
                ui_text::mono(wormhole_i18n::t(group.i18n_key), font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(18.0)
            .finish(),
        );
        for key in group.keys {
            let hex = active
                .colors
                .get(*key)
                .cloned()
                .unwrap_or_else(|| color_u_to_hex(palette.color(key)));
            let color = theme::parse_hex(&hex).unwrap_or_else(|| palette.color(key));
            let editing = studio.token_key.as_deref() == Some(*key);
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            row.add_child(swatch(color));
            row.add_child(
                Container::new(
                    ui_text::body((*key).to_string(), font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_margin_left(10.0)
                .finish(),
            );
            row.add_child(
                Container::new(
                    ui_text::mono(hex, font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_left(12.0)
                .finish(),
            );
            let key_owned = (*key).to_string();
            row.add_child(
                Container::new(chip_button(
                    if editing {
                        wormhole_i18n::t("settings.theme.editing")
                    } else {
                        wormhole_i18n::t("settings.theme.edit")
                    },
                    editing,
                    ThemeStudioAction::BeginEditToken(key_owned.clone()),
                    font,
                    format!("settings:theme:token:{key_owned}"),
                ))
                .with_margin_left(12.0)
                .finish(),
            );
            col.add_child(Container::new(row.finish()).with_margin_top(8.0).finish());
            // Inline hex editor under the token being edited (stays in first viewport).
            if editing {
                let mut hex_row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);
                hex_row.add_child(Expanded::new(
                    1.0,
                    text_field(
                        &studio.token_draft,
                        &studio.token_field,
                        studio.token_focused,
                        "#RRGGBB",
                        font,
                        "settings:theme:hex_field",
                        "theme hex",
                        ThemeStudioAction::FocusTokenField,
                        ThemeStudioAction::TokenFieldEdit,
                    ),
                )
                .finish());
                hex_row.add_child(
                    Container::new(chip_button(
                        wormhole_i18n::t("settings.theme.apply_hex"),
                        true,
                        ThemeStudioAction::ApplyTokenHex,
                        font,
                        "settings:theme:apply_hex".into(),
                    ))
                    .with_margin_left(8.0)
                    .finish(),
                );
                col.add_child(
                    Container::new(hex_row.finish())
                        .with_margin_top(6.0)
                        .with_margin_bottom(4.0)
                        .finish(),
                );
            }
        }
    }

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.fonts_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(18.0)
        .finish(),
    );
    for role in FontRole::ALL {
        let current = active
            .fonts
            .get(role.as_str())
            .cloned()
            .unwrap_or_else(|| theme::tokens::default_font_for_role(role).to_string());
        col.add_child(
            Container::new(
                ui_text::body(role.as_str().to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
        let mut font_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Min);
        for choice in FONT_CHOICES {
            let selected = current == choice.id;
            font_row.add_child(
                Container::new(chip_button(
                    choice.label.to_string(),
                    selected,
                    ThemeStudioAction::SetFontRole {
                        role: role.as_str().to_string(),
                        choice: choice.id.to_string(),
                    },
                    font,
                    format!("settings:theme:font:{}:{}", role.as_str(), choice.id),
                ))
                .with_margin_right(6.0)
                .with_margin_top(6.0)
                .finish(),
            );
        }
        col.add_child(font_row.finish());
    }

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.backdrop_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(18.0)
        .finish(),
    );
    let mut backdrop_row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for (kind, label_key, auto_id) in [
        (
            BackdropKind::Mesh,
            "settings.theme.backdrop.mesh",
            "settings:theme:backdrop:mesh",
        ),
        (
            BackdropKind::Solid,
            "settings.theme.backdrop.solid",
            "settings:theme:backdrop:solid",
        ),
        (
            BackdropKind::Image,
            "settings.theme.backdrop.image",
            "settings:theme:backdrop:image",
        ),
    ] {
        let selected = active.backdrop.kind == kind;
        backdrop_row.add_child(
            Container::new(chip_button(
                wormhole_i18n::t(label_key),
                selected,
                ThemeStudioAction::SetBackdropKind(kind),
                font,
                auto_id.into(),
            ))
            .with_margin_right(8.0)
            .with_margin_top(8.0)
            .finish(),
        );
    }
    col.add_child(backdrop_row.finish());
    col.add_child(
        Container::new(chip_button(
            if active.backdrop.dots {
                wormhole_i18n::t("settings.theme.dots_on")
            } else {
                wormhole_i18n::t("settings.theme.dots_off")
            },
            active.backdrop.dots,
            ThemeStudioAction::ToggleBackdropDots,
            font,
            "settings:theme:dots".into(),
        ))
        .with_margin_top(8.0)
        .finish(),
    );
    if matches!(active.backdrop.kind, BackdropKind::Image) {
        col.add_child(
            Container::new(
                ui_text::mono(wormhole_i18n::t("settings.theme.image_hint"), font)
                    .with_color(theme::placeholder())
                    .finish(),
            )
            .with_margin_top(6.0)
            .finish(),
        );
    }

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.manage_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(18.0)
        .finish(),
    );
    let mut manage = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for (label_key, action, id) in [
        (
            "settings.theme.new",
            ThemeStudioAction::NewBlankCustom,
            "settings:theme:new",
        ),
        (
            "settings.theme.reset",
            ThemeStudioAction::ResetActive,
            "settings:theme:reset",
        ),
        (
            "settings.theme.delete",
            ThemeStudioAction::DeleteActive,
            "settings:theme:delete",
        ),
        (
            "settings.theme.export",
            ThemeStudioAction::ExportActive,
            "settings:theme:export",
        ),
        (
            "settings.theme.import_clipboard",
            ThemeStudioAction::ImportClipboard,
            "settings:theme:import_clipboard",
        ),
    ] {
        manage.add_child(
            Container::new(chip_button(
                wormhole_i18n::t(label_key),
                false,
                action,
                font,
                id.into(),
            ))
            .with_margin_right(8.0)
            .with_margin_top(8.0)
            .finish(),
        );
    }
    col.add_child(manage.finish());

    col.add_child(
        Container::new(
            ui_text::mono(wormhole_i18n::t("settings.theme.import_label"), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_margin_top(12.0)
        .finish(),
    );
    col.add_child(
        Container::new(text_field(
            &studio.import_draft,
            &studio.import_field,
            studio.import_focused,
            "{ … theme json … }",
            font,
            "settings:theme:import_field",
            "theme import json",
            ThemeStudioAction::FocusImport,
            ThemeStudioAction::ImportFieldEdit,
        ))
        .with_margin_top(6.0)
        .finish(),
    );
    col.add_child(
        Container::new(chip_button(
            wormhole_i18n::t("settings.theme.import_apply"),
            true,
            ThemeStudioAction::ImportDraft,
            font,
            "settings:theme:import_apply".into(),
        ))
        .with_margin_top(8.0)
        .finish(),
    );

    if !studio.export_preview.is_empty() {
        col.add_child(
            Container::new(
                ui_text::mono(studio.export_preview.clone(), font)
                    .with_color(theme::placeholder())
                    .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
    }

    if !studio.message.is_empty() {
        col.add_child(
            Container::new(status_line(
                studio.message.clone(),
                font,
                studio.tone,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
    }

    Container::new(col.finish())
        .with_padding_top(4.0)
        .with_padding_bottom(8.0)
        .finish()
}

pub fn handle_action_with_app(
    state: &mut ThemeStudioState,
    data_dir: &Path,
    action: ThemeStudioAction,
    app: &warpui::AppContext,
) {
    handle_action(state, data_dir, action, system_is_dark_from_app(app));
}
