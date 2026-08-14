//! Settings → Exit Node: import `vless://` (Reality + Vision) and start Xray TUN.

use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::exit_node::ExitNodeRuntimeStatus;
use wormhole_desktop_core::exit_node_commands::{
    exit_node_clear, exit_node_import_uri, exit_node_start, exit_node_status, exit_node_stop,
    ExitNodeImportParams,
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
pub enum ExitNodeAction {
    Refresh,
    Import,
    Start,
    Stop,
    Clear,
    FocusUri,
    UriEdit(TextFieldEditAction),
}

pub struct ExitNodeView {
    core: CoreHandle,
    font: FamilyId,
    uri_draft: String,
    uri_field: TextFieldState,
    uri_focused: bool,
    status: Option<ExitNodeRuntimeStatus>,
    message: String,
    tone: StatusTone,
    busy: bool,
}

impl ExitNodeView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            uri_draft: String::new(),
            uri_field: TextFieldState::new(),
            uri_focused: false,
            status: None,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh(ctx);
        view
    }

    fn apply_status(&mut self, dto: ExitNodeRuntimeStatus) {
        self.status = Some(dto);
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        self.message = wormhole_i18n::t("settings.exit_node.loading");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                exit_node_status(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_status(dto);
                        view.message = wormhole_i18n::t("settings.exit_node.ready");
                        view.tone = StatusTone::Placeholder;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.exit_node.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn import_uri(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        let uri = self.uri_draft.trim().to_string();
        if uri.is_empty() {
            self.message = wormhole_i18n::t("settings.exit_node.uri_empty");
            self.tone = StatusTone::Danger;
            ctx.notify();
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.exit_node.importing");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        let params = ExitNodeImportParams { uri };
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                exit_node_import_uri(&state, params).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_status(dto);
                        view.uri_draft.clear();
                        view.message = wormhole_i18n::t("settings.exit_node.imported");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.exit_node.import_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn start(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.exit_node.starting");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                exit_node_start(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_status(dto);
                        view.message = wormhole_i18n::t("settings.exit_node.started");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.exit_node.start_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn stop(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.exit_node.stopping");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                exit_node_stop(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_status(dto);
                        view.message = wormhole_i18n::t("settings.exit_node.stopped");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.exit_node.stop_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn clear(&mut self, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.message = wormhole_i18n::t("settings.exit_node.clearing");
        self.tone = StatusTone::Placeholder;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                exit_node_clear(&state).await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(dto) => {
                        view.apply_status(dto);
                        view.message = wormhole_i18n::t("settings.exit_node.cleared");
                        view.tone = StatusTone::Success;
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.exit_node.clear_failed")
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
        action: ExitNodeAction,
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

    fn summary_text(&self) -> String {
        let Some(s) = self.status.as_ref() else {
            return wormhole_i18n::t("settings.exit_node.summary_none");
        };
        let mut lines = Vec::new();
        if s.running {
            lines.push(format!(
                "{} (pid {})",
                wormhole_i18n::t("settings.exit_node.summary_running"),
                s.pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
            ));
        } else {
            lines.push(wormhole_i18n::t("settings.exit_node.summary_stopped"));
        }
        if let Some(host) = s.host.as_ref() {
            let port = s.port.unwrap_or(0);
            let remark = s.remark.as_deref().unwrap_or("");
            if remark.is_empty() {
                lines.push(format!("{host}:{port}"));
            } else {
                lines.push(format!("{remark} — {host}:{port}"));
            }
        } else if s.uri_configured {
            lines.push(wormhole_i18n::t("settings.exit_node.summary_uri_set"));
        } else {
            lines.push(wormhole_i18n::t("settings.exit_node.summary_uri_missing"));
        }
        if s.xray_found {
            if let Some(path) = s.xray_path.as_ref() {
                lines.push(format!(
                    "{} {path}",
                    wormhole_i18n::t("settings.exit_node.summary_xray")
                ));
            }
        } else {
            lines.push(wormhole_i18n::t("settings.exit_node.summary_xray_missing"));
        }
        lines.join("\n")
    }
}

impl Entity for ExitNodeView {
    type Event = ();
}

impl TypedActionView for ExitNodeView {
    type Action = ExitNodeAction;

    fn handle_action(&mut self, action: &ExitNodeAction, ctx: &mut ViewContext<Self>) {
        match action {
            ExitNodeAction::Refresh => self.refresh(ctx),
            ExitNodeAction::Import => self.import_uri(ctx),
            ExitNodeAction::Start => self.start(ctx),
            ExitNodeAction::Stop => self.stop(ctx),
            ExitNodeAction::Clear => self.clear(ctx),
            ExitNodeAction::FocusUri => {
                self.uri_focused = true;
                ctx.notify();
            }
            ExitNodeAction::UriEdit(edit) => {
                self.uri_field.apply(&mut self.uri_draft, edit);
                ctx.notify();
            }
        }
    }
}

impl View for ExitNodeView {
    fn ui_name() -> &'static str {
        "ExitNodeView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let running = self.status.as_ref().map(|s| s.running).unwrap_or(false);
        let configured = self
            .status
            .as_ref()
            .map(|s| s.uri_configured)
            .unwrap_or(false);

        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_title(
            wormhole_i18n::t("settings.exit_node.title"),
            self.font,
        ));
        col.add_child(section_hint(
            wormhole_i18n::t("settings.exit_node.hint"),
            self.font,
        ));
        col.add_child(
            EventHandler::new(status_line(self.message.clone(), self.font, self.tone))
                .with_automation_id("settings:exit_node_status")
                .with_automation_label(if self.message.is_empty() {
                    wormhole_i18n::t("settings.exit_node.status_label")
                } else {
                    self.message.clone()
                })
                .finish(),
        );

        let mut summary_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        summary_col.add_child(section_title(
            wormhole_i18n::t("settings.exit_node.summary_title"),
            self.font,
        ));
        summary_col.add_child(
            Container::new(
                ui_text::body(self.summary_text(), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(section_card(summary_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let rendered = render_field_with_caret(
            &self.uri_draft,
            &self.uri_field.marked_text,
            &wormhole_i18n::t("settings.exit_node.uri_placeholder"),
            self.font,
            self.uri_focused,
            false,
            false,
            self.uri_field.cursor,
        );
        let input = TextFieldInput::builder(rendered, |ctx, action| {
            ctx.dispatch_typed_action(ExitNodeAction::UriEdit(action));
        })
        .focused(self.uri_focused)
        .ime_preedit(!self.uri_field.marked_text.is_empty())
        .finish();
        let input = wrap_text_field_focus_on_click(input, |ctx| {
            ctx.dispatch_typed_action(ExitNodeAction::FocusUri);
        });

        let mut import_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        import_col.add_child(section_title(
            wormhole_i18n::t("settings.exit_node.import_title"),
            self.font,
        ));
        import_col.add_child(
            EventHandler::new(
                Container::new(input)
                    .with_uniform_padding(10.0)
                    .with_margin_top(8.0)
                    .with_background(theme::canvas())
                    .with_border(Border::all(1.0).with_border_fill(if self.uri_focused {
                        theme::accent_cool()
                    } else {
                        theme::border()
                    }))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.0)))
                    .finish(),
            )
            .with_automation_id("exit-node-uri")
            .with_automation_label(wormhole_i18n::t("settings.exit_node.uri_label"))
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish(),
        );
        let mut import_row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        import_row.add_child(self.action_chip(
            &wormhole_i18n::t("settings.exit_node.import"),
            ExitNodeAction::Import,
            true,
            "exit-node-import",
            self.busy,
        ));
        import_row.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.exit_node.refresh"),
                ExitNodeAction::Refresh,
                false,
                "exit-node-refresh",
                self.busy,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        import_col.add_child(
            Container::new(import_row.finish())
                .with_margin_top(12.0)
                .finish(),
        );
        col.add_child(
            Container::new(section_card(import_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        let mut ctrl_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        ctrl_col.add_child(section_title(
            wormhole_i18n::t("settings.exit_node.control_title"),
            self.font,
        ));
        let mut ctrl_row = Flex::row().with_main_axis_size(MainAxisSize::Max);
        ctrl_row.add_child(self.action_chip(
            &wormhole_i18n::t("settings.exit_node.start"),
            ExitNodeAction::Start,
            true,
            "exit-node-start",
            self.busy || running || !configured,
        ));
        ctrl_row.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.exit_node.stop"),
                ExitNodeAction::Stop,
                false,
                "exit-node-stop",
                self.busy || !running,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        ctrl_row.add_child(
            Container::new(self.action_chip(
                &wormhole_i18n::t("settings.exit_node.clear"),
                ExitNodeAction::Clear,
                false,
                "exit-node-clear",
                self.busy || !configured,
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        ctrl_col.add_child(
            Container::new(ctrl_row.finish())
                .with_margin_top(8.0)
                .finish(),
        );
        ctrl_col.add_child(
            Container::new(
                ui_text::body(
                    wormhole_i18n::t("settings.exit_node.admin_hint"),
                    self.font,
                )
                .with_color(theme::muted())
                .finish(),
            )
            .with_margin_top(10.0)
            .finish(),
        );
        col.add_child(
            Container::new(section_card(ctrl_col.finish()))
                .with_margin_top(SECTION_GAP)
                .finish(),
        );

        col.finish()
    }
}
