//! Settings → File Search: OpenAI BYOK vector store for Agent `file_search`.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::file_search::VectorStoreFileDto;
use wormhole_desktop_core::file_search_commands::{
    file_search_config, file_search_configure, file_search_list, file_search_remove,
    file_search_upload, FileSearchConfigDto, FileSearchConfigureParams, FileSearchRemoveParams,
    FileSearchUploadParams,
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
pub enum FileSearchAction {
    Refresh,
    Save,
    ToggleEnabled,
    FocusKey,
    KeyEdit(TextFieldEditAction),
    FocusPath,
    PathEdit(TextFieldEditAction),
    FocusMaxResults,
    MaxResultsEdit(TextFieldEditAction),
    Upload,
    Remove(String),
}

pub struct FileSearchView {
    core: CoreHandle,
    font: FamilyId,
    enabled: bool,
    openai_configured: bool,
    vector_store_id: Option<String>,
    max_num_results: usize,
    config_path: String,
    key_draft: String,
    key_field: TextFieldState,
    key_focused: bool,
    path_draft: String,
    path_field: TextFieldState,
    path_focused: bool,
    max_results_draft: String,
    max_results_field: TextFieldState,
    max_results_focused: bool,
    files: Vec<VectorStoreFileDto>,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl FileSearchView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            enabled: false,
            openai_configured: false,
            vector_store_id: None,
            max_num_results: 8,
            config_path: String::new(),
            key_draft: String::new(),
            key_field: TextFieldState::new(),
            key_focused: false,
            path_draft: String::new(),
            path_field: TextFieldState::new(),
            path_focused: false,
            max_results_draft: "8".into(),
            max_results_field: TextFieldState::new(),
            max_results_focused: false,
            files: Vec::new(),
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn apply_dto(&mut self, dto: FileSearchConfigDto) {
        self.enabled = dto.enabled;
        self.openai_configured = dto.openai_configured;
        self.vector_store_id = dto.vector_store_id;
        self.max_num_results = dto.max_num_results;
        self.max_results_draft = dto.max_num_results.to_string();
        self.config_path = dto.config_path;
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.file_search.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let dto = file_search_config(&state).await?;
                let files = if dto.enabled && dto.openai_configured {
                    file_search_list(&state).await.unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok::<_, String>((dto, files))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((dto, files)) => {
                        view.apply_dto(dto);
                        view.files = files;
                        view.message = wormhole_i18n::t("settings.file_search.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.file_search.load_failed")
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
        self.message = wormhole_i18n::t("settings.file_search.saving");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let max_num_results = self
            .max_results_draft
            .trim()
            .parse::<usize>()
            .ok()
            .or(Some(self.max_num_results));
        let params = FileSearchConfigureParams {
            enabled: Some(self.enabled),
            openai_api_key: {
                let t = self.key_draft.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            },
            max_num_results,
            base_url: None,
            clear_openai_api_key: false,
        };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let dto = file_search_configure(&state, params).await?;
                let files = if dto.enabled && dto.openai_configured {
                    file_search_list(&state).await.unwrap_or_default()
                } else {
                    Vec::new()
                };
                Ok::<_, String>((dto, files))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((dto, files)) => {
                        view.apply_dto(dto);
                        view.files = files;
                        view.key_draft.clear();
                        view.message = wormhole_i18n::t("settings.file_search.saved");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.file_search.save_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn upload(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let path = self.path_draft.trim().to_string();
        if path.is_empty() {
            self.message = wormhole_i18n::t("settings.file_search.path_required");
            self.tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.file_search.uploading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let result = file_search_upload(
                    &state,
                    FileSearchUploadParams { path },
                )
                .await?;
                let files = file_search_list(&state).await.unwrap_or_default();
                let dto = file_search_config(&state).await?;
                Ok::<_, String>((result, files, dto))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((result, files, dto)) => {
                        view.apply_dto(dto);
                        view.files = files;
                        view.path_draft.clear();
                        view.message = format!(
                            "{} {} ({})",
                            wormhole_i18n::t("settings.file_search.uploaded"),
                            result.filename.unwrap_or(result.file_id),
                            result.status
                        );
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.file_search.upload_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn remove_file(&mut self, ctx: &mut ViewContext<Self>, file_id: String) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.file_search.removing");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                file_search_remove(
                    &state,
                    FileSearchRemoveParams { file_id },
                )
                .await?;
                let files = file_search_list(&state).await.unwrap_or_default();
                Ok::<_, String>(files)
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(files) => {
                        view.files = files;
                        view.message = wormhole_i18n::t("settings.file_search.removed");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.file_search.remove_failed")
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
        action: FileSearchAction,
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
        focus_action: FileSearchAction,
        edit_to_action: fn(TextFieldEditAction) -> FileSearchAction,
        automation_id: &str,
        placeholder: &str,
        trailing: Option<String>,
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
        let mut header = Flex::row().with_main_axis_size(MainAxisSize::Max).with_child(
            ui_text::body(label.to_string(), self.font)
                .with_color(theme::text())
                .finish(),
        );
        if let Some(trail) = trailing {
            header.add_child(
                Container::new(
                    ui_text::body(trail, self.font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_margin_left(8.0)
                .finish(),
            );
        }
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(header.finish())
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

    fn file_row(&self, file: &VectorStoreFileDto) -> Box<dyn Element> {
        let name = file
            .filename
            .clone()
            .unwrap_or_else(|| file.id.clone());
        let status = file.status.clone();
        let file_id = file.id.clone();
        let remove_id = format!("settings:file_search_remove_{}", file.id);
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Flex::column()
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(
                        ui_text::body(name, self.font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_child(
                        Container::new(
                            ui_text::body(
                                format!("{} · {}", status, file.id),
                                self.font,
                            )
                            .with_color(theme::muted())
                            .finish(),
                        )
                        .with_margin_top(2.0)
                        .finish(),
                    )
                    .finish(),
            )
            .with_child(
                Container::new(self.action_chip(
                    &wormhole_i18n::t("settings.file_search.remove"),
                    FileSearchAction::Remove(file_id),
                    false,
                    &remove_id,
                    self.busy,
                ))
                .with_margin_left(12.0)
                .finish(),
            )
            .finish()
    }
}

impl Entity for FileSearchView {
    type Event = ();
}

impl TypedActionView for FileSearchView {
    type Action = FileSearchAction;

    fn handle_action(&mut self, action: &FileSearchAction, ctx: &mut ViewContext<Self>) {
        match action {
            FileSearchAction::Refresh => self.refresh(ctx),
            FileSearchAction::Save => self.save(ctx),
            FileSearchAction::ToggleEnabled => {
                self.enabled = !self.enabled;
                ctx.notify();
            }
            FileSearchAction::FocusKey => {
                self.key_focused = true;
                self.path_focused = false;
                self.max_results_focused = false;
                ctx.notify();
            }
            FileSearchAction::KeyEdit(edit) => {
                self.key_field.apply(&mut self.key_draft, edit);
                ctx.notify();
            }
            FileSearchAction::FocusPath => {
                self.path_focused = true;
                self.key_focused = false;
                self.max_results_focused = false;
                ctx.notify();
            }
            FileSearchAction::PathEdit(edit) => {
                self.path_field.apply(&mut self.path_draft, edit);
                ctx.notify();
            }
            FileSearchAction::FocusMaxResults => {
                self.max_results_focused = true;
                self.key_focused = false;
                self.path_focused = false;
                ctx.notify();
            }
            FileSearchAction::MaxResultsEdit(edit) => {
                self.max_results_field
                    .apply(&mut self.max_results_draft, edit);
                ctx.notify();
            }
            FileSearchAction::Upload => self.upload(ctx),
            FileSearchAction::Remove(file_id) => self.remove_file(ctx, file_id.clone()),
        }
    }
}

impl View for FileSearchView {
    fn ui_name() -> &'static str {
        "FileSearchView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.file_search.title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.file_search.hint"),
            self.font,
        ));
        col.add_child(
            EventHandler::new(status_line(self.message.clone(), self.font, self.tone))
                .with_automation_id("settings:file_search_status")
                .with_automation_label(wormhole_i18n::t("settings.file_search.status_label"))
                .finish(),
        );

        let mut enable_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        enable_col.add_child(section_title(
            wormhole_i18n::t("settings.file_search.enable_title"),
            self.font,
        ));
        enable_col.add_child(
            Container::new(self.action_chip(
                &if self.enabled {
                    wormhole_i18n::t("settings.file_search.enabled")
                } else {
                    wormhole_i18n::t("settings.file_search.disabled")
                },
                FileSearchAction::ToggleEnabled,
                self.enabled,
                "settings:file_search_toggle",
                self.busy,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(section_card(enable_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let key_status = if self.openai_configured {
            wormhole_i18n::t("settings.file_search.key_set")
        } else {
            wormhole_i18n::t("settings.file_search.key_missing")
        };
        let mut keys_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        keys_col.add_child(section_title(
            wormhole_i18n::t("settings.file_search.keys_title"),
            self.font,
        ));
        keys_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.file_search.openai_key"),
                &self.key_draft,
                &self.key_field,
                self.key_focused,
                FileSearchAction::FocusKey,
                FileSearchAction::KeyEdit,
                "settings:file_search_openai_key",
                &wormhole_i18n::t("settings.file_search.key_placeholder"),
                Some(key_status),
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        keys_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.file_search.max_results"),
                &self.max_results_draft,
                &self.max_results_field,
                self.max_results_focused,
                FileSearchAction::FocusMaxResults,
                FileSearchAction::MaxResultsEdit,
                "settings:file_search_max_results",
                "8",
                None,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
        if let Some(vs) = &self.vector_store_id {
            keys_col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{} {vs}",
                            wormhole_i18n::t("settings.file_search.vector_store")
                        ),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(12.0)
                .finish(),
            );
        }
        col.add_child(
            Container::new(section_card(keys_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut upload_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        upload_col.add_child(section_title(
            wormhole_i18n::t("settings.file_search.upload_title"),
            self.font,
        ));
        upload_col.add_child(
            Container::new(self.text_field(
                &wormhole_i18n::t("settings.file_search.path"),
                &self.path_draft,
                &self.path_field,
                self.path_focused,
                FileSearchAction::FocusPath,
                FileSearchAction::PathEdit,
                "settings:file_search_path",
                &wormhole_i18n::t("settings.file_search.path_placeholder"),
                None,
            ))
            .with_margin_top(8.0)
            .finish(),
        );
        upload_col.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.file_search.upload"),
                FileSearchAction::Upload,
                true,
                "settings:file_search_upload",
                self.busy,
            ))
            .with_margin_top(12.0)
            .finish(),
        );
        col.add_child(
            Container::new(section_card(upload_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut files_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        files_col.add_child(section_title(
            wormhole_i18n::t("settings.file_search.files_title"),
            self.font,
        ));
        if self.files.is_empty() {
            files_col.add_child(
                Container::new(
                    ui_text::body(
                        wormhole_i18n::t("settings.file_search.files_empty"),
                        self.font,
                    )
                    .with_color(theme::muted())
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        } else {
            for file in &self.files {
                files_col.add_child(
                    Container::new(self.file_row(file))
                        .with_margin_top(8.0)
                        .finish(),
                );
            }
        }
        col.add_child(
            Container::new(section_card(files_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        if !self.config_path.is_empty() {
            col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{} {}",
                            wormhole_i18n::t("settings.file_search.config_path"),
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
            &wormhole_i18n::t("settings.file_search.refresh"),
            FileSearchAction::Refresh,
            false,
            "settings:file_search_refresh",
            self.busy,
        ));
        actions.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.file_search.save"),
                FileSearchAction::Save,
                true,
                "settings:file_search_save",
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
