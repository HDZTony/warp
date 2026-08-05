//! Settings → Memory：OpenHuman Brain 图谱整页（Warp 原生）。
//!
//! Tabs: Graph（tree/contacts 导出）| Browse（检索/下钻）| Vault（路径与 sources）。
//! 不用 Pixi/WebGL：以层级节点列表 + 详情复刻图谱产品面。

use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};
use wormhole_desktop_core::memory_commands::{
    memory_graph_export, memory_ingest_agent_transcripts, memory_open_obsidian, memory_persona_run,
    memory_persona_status, memory_reindex, memory_reveal_folder, memory_rss_seed_us_markets,
    memory_rss_sync, memory_sources_list, memory_sources_remove, memory_sources_upsert,
    memory_status, memory_tree, GraphExport, GraphMode, GraphNode, MemoryGraphExportParams,
    MemoryPersonaRunParams, MemorySource, MemorySourceKind, MemorySourcesRemoveParams,
    MemorySourcesUpsertParams, MemoryStatus, MemoryTreeMode, MemoryTreeQuery,
};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{section_hint, status_line, StatusTone};
use crate::ui::text_field_input::{
    render_field_with_caret, TextFieldEditAction, TextFieldInput, TextFieldState,
};
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryPane {
    Graph,
    Browse,
    Vault,
}

#[derive(Debug, Clone)]
pub enum MemoryAction {
    SelectPane(u8),
    SetGraphMode(GraphMode),
    Refresh,
    RefreshGraph,
    OpenObsidian,
    RevealVault,
    Reindex,
    IngestAgentTranscripts,
    SyncRss,
    SeedUsMarketRss,
    RunPersona,
    SelectNode(String),
    BrowseSearchEdit(TextFieldEditAction),
    FocusBrowseSearch,
    RunBrowseSearch,
    SelectBrowseHit(String),
    DrillSelected,
    SourcePathEdit(TextFieldEditAction),
    FocusSourcePath,
    AddSource,
    RemoveSource(String),
}

pub struct MemoryView {
    core: CoreHandle,
    font: FamilyId,
    pane: MemoryPane,
    status: Option<MemoryStatus>,
    sources: Vec<MemorySource>,
    graph: Option<GraphExport>,
    graph_mode: GraphMode,
    selected_node_id: Option<String>,
    browse_query: String,
    browse_field: TextFieldState,
    browse_focused: bool,
    browse_hits: Vec<BrowseHit>,
    selected_hit_id: Option<String>,
    hit_detail: String,
    source_draft: String,
    source_field: TextFieldState,
    source_focused: bool,
    message: String,
    tone: StatusTone,
    busy: bool,
}

#[derive(Debug, Clone)]
struct BrowseHit {
    id: String,
    label: String,
}

impl MemoryView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            pane: MemoryPane::Graph,
            status: None,
            sources: Vec::new(),
            graph: None,
            graph_mode: GraphMode::Tree,
            selected_node_id: None,
            browse_query: String::new(),
            browse_field: TextFieldState::new(),
            browse_focused: false,
            browse_hits: Vec::new(),
            selected_hit_id: None,
            hit_detail: String::new(),
            source_draft: String::new(),
            source_field: TextFieldState::new(),
            source_focused: false,
            message: String::new(),
            tone: StatusTone::Placeholder,
            busy: false,
        };
        view.refresh_all(ctx);
        view
    }

    fn refresh_all(&mut self, ctx: &mut ViewContext<Self>) {
        self.refresh_status(ctx);
        self.refresh_graph(ctx);
    }

    fn refresh_status(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                let status = memory_status(&state).await?;
                let sources = memory_sources_list(&state).await?;
                Ok::<_, String>((status, sources))
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok((status, sources)) => {
                        view.status = Some(status);
                        view.sources = sources;
                        if view.message.is_empty() {
                            view.message = wormhole_i18n::t("settings.memory.ready");
                            view.tone = StatusTone::Placeholder;
                        }
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.memory.load_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn refresh_graph(&mut self, ctx: &mut ViewContext<Self>) {
        self.busy = true;
        ctx.notify();
        let core = self.core.clone();
        let mode = self.graph_mode;
        ctx.spawn(
            async move {
                let state = core.runtime().state.clone();
                memory_graph_export(
                    &state,
                    MemoryGraphExportParams { mode },
                )
                .await
            },
            |view, output, ctx| {
                view.busy = false;
                match output {
                    Ok(graph) => {
                        view.message = format!(
                            "{} · nodes={} · edges={}",
                            wormhole_i18n::t("settings.memory.graph_loaded"),
                            graph.nodes.len(),
                            graph.edges.len()
                        );
                        view.tone = StatusTone::Success;
                        if view
                            .selected_node_id
                            .as_ref()
                            .is_some_and(|id| !graph.nodes.iter().any(|n| &n.id == id))
                        {
                            view.selected_node_id = None;
                        }
                        view.graph = Some(graph);
                    }
                    Err(error) => {
                        view.message = format!(
                            "{}: {error}",
                            wormhole_i18n::t("settings.memory.graph_failed")
                        );
                        view.tone = StatusTone::Danger;
                    }
                }
                ctx.notify();
            },
        );
    }

    fn action_button(
        &self,
        label: &str,
        action: MemoryAction,
        disabled: bool,
        primary: bool,
        automation_id: &str,
    ) -> Box<dyn Element> {
        let label = label.to_string();
        Container::new(
            EventHandler::new(
                ui_text::body(label.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(automation_id)
            .on_left_mouse_down(move |ctx, _, _| {
                if disabled {
                    return DispatchEventResult::StopPropagation;
                }
                ctx.dispatch_typed_action(action.clone());
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

    fn pane_tab(&self, pane: MemoryPane, label_key: &str, id: &str) -> Box<dyn Element> {
        let selected = self.pane == pane;
        let label = wormhole_i18n::t(label_key);
        let index = match pane {
            MemoryPane::Graph => 0u8,
            MemoryPane::Browse => 1,
            MemoryPane::Vault => 2,
        };
        Container::new(
            EventHandler::new(
                ui_text::body(
                    if selected {
                        format!("● {label}")
                    } else {
                        format!("○ {label}")
                    },
                    self.font,
                )
                .with_color(if selected {
                    theme::text()
                } else {
                    theme::muted()
                })
                .finish(),
            )
            .with_automation_label(label)
            .with_automation_id(id)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(MemoryAction::SelectPane(index));
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(if selected {
            theme::accent_cool_bg(36)
        } else {
            theme::bg()
        })
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
    }

    fn toolbar(&self) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        row.add_child(self.pane_tab(
            MemoryPane::Graph,
            "settings.memory.tab_graph",
            "settings:memory_tab_graph",
        ));
        row.add_child(
            Container::new(self.pane_tab(
                MemoryPane::Browse,
                "settings.memory.tab_browse",
                "settings:memory_tab_browse",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        row.add_child(
            Container::new(self.pane_tab(
                MemoryPane::Vault,
                "settings.memory.tab_vault",
                "settings:memory_tab_vault",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        row.add_child(Expanded::new(1.0, Flex::row().finish()).finish());
        row.add_child(self.action_button(
            &wormhole_i18n::t("settings.memory.refresh"),
            MemoryAction::Refresh,
            self.busy,
            false,
            "settings:memory_btn_refresh",
        ));
        Container::new(row.finish()).with_margin_bottom(12.0).finish()
    }

    fn graph_pane(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.memory.graph_hint"),
            self.font,
        ));
        let mut mode_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        mode_row.add_child(self.action_button(
            &wormhole_i18n::t("settings.memory.graph_mode_tree"),
            MemoryAction::SetGraphMode(GraphMode::Tree),
            self.busy || self.graph_mode == GraphMode::Tree,
            self.graph_mode == GraphMode::Tree,
            "settings:memory_graph_mode_tree",
        ));
        mode_row.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.graph_mode_contacts"),
                MemoryAction::SetGraphMode(GraphMode::Contacts),
                self.busy || self.graph_mode == GraphMode::Contacts,
                self.graph_mode == GraphMode::Contacts,
                "settings:memory_graph_mode_contacts",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        mode_row.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.graph_refresh"),
                MemoryAction::RefreshGraph,
                self.busy,
                true,
                "settings:memory_graph_refresh",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        mode_row.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.open_obsidian"),
                MemoryAction::OpenObsidian,
                self.busy,
                false,
                "settings:memory_open_obsidian",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(mode_row.finish())
                .with_margin_top(10.0)
                .finish(),
        );

        let mut body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        body.add_child(Expanded::new(1.0, self.graph_node_list()).finish());
        body.add_child(
            ConstrainedBox::new(self.graph_detail())
                .with_width(320.0)
                .finish(),
        );
        col.add_child(
            EventHandler::new(
                Container::new(body.finish())
                    .with_margin_top(12.0)
                    .finish(),
            )
            .with_automation_id("settings:memory_graph")
            .with_automation_label(wormhole_i18n::t("settings.memory.tab_graph"))
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish(),
        );
        col.finish()
    }

    fn graph_node_list(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        let Some(graph) = &self.graph else {
            col.add_child(
                ui_text::body(wormhole_i18n::t("settings.memory.graph_empty"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
            return Container::new(col.finish())
                .with_uniform_padding(10.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .finish();
        };
        col.add_child(
            ui_text::mono(
                format!(
                    "{} · {} nodes · {} edges",
                    match graph.mode {
                        GraphMode::Tree => "tree",
                        GraphMode::Contacts => "contacts",
                    },
                    graph.nodes.len(),
                    graph.edges.len()
                ),
                self.font,
            )
            .with_color(theme::muted())
            .finish(),
        );
        let mut nodes = graph.nodes.clone();
        nodes.sort_by(|a, b| {
            kind_rank(&a.kind)
                .cmp(&kind_rank(&b.kind))
                .then(a.level.unwrap_or(999).cmp(&b.level.unwrap_or(999)))
                .then(a.label.cmp(&b.label))
        });
        for node in nodes.into_iter().take(400) {
            let selected = self.selected_node_id.as_deref() == Some(node.id.as_str());
            let id = node.id.clone();
            let label = format!(
                "[{}] {}{}",
                node.kind,
                node.label.chars().take(64).collect::<String>(),
                node.level
                    .map(|l| format!(" · L{l}"))
                    .unwrap_or_default()
            );
            col.add_child(
                Container::new(
                    EventHandler::new(
                        ui_text::mono(label, self.font)
                            .with_color(if selected {
                                theme::text()
                            } else {
                                theme::muted()
                            })
                            .finish(),
                    )
                    .with_automation_label(node.id.clone())
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(MemoryAction::SelectNode(id.clone()));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                )
                .with_margin_top(4.0)
                .with_uniform_padding(6.0)
                .with_background(if selected {
                    theme::accent_cool_bg(40)
                } else {
                    theme::panel_elevated()
                })
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
            );
        }
        if graph.nodes.len() > 400 {
            col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{} (+{} more)",
                            wormhole_i18n::t("settings.memory.graph_truncated"),
                            graph.nodes.len() - 400
                        ),
                        self.font,
                    )
                    .with_color(theme::placeholder())
                    .finish(),
                )
                .with_margin_top(8.0)
                .finish(),
            );
        }
        Container::new(col.finish())
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .with_margin_right(10.0)
            .finish()
    }

    fn graph_detail(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(
            ui_text::body(wormhole_i18n::t("settings.memory.graph_detail"), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        match (self.graph.as_ref(), self.selected_node_id.as_deref()) {
            (Some(graph), Some(id)) => {
                if let Some(node) = graph.nodes.iter().find(|n| n.id == id) {
                    col.add_child(
                        Container::new(
                            ui_text::mono(format_node_detail(node), self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .with_margin_top(8.0)
                        .finish(),
                    );
                    let children = related_nodes(graph, id);
                    if !children.is_empty() {
                        col.add_child(
                            Container::new(
                                ui_text::body(
                                    format!(
                                        "{} ({})",
                                        wormhole_i18n::t("settings.memory.graph_related"),
                                        children.len()
                                    ),
                                    self.font,
                                )
                                .with_color(theme::muted())
                                .finish(),
                            )
                            .with_margin_top(12.0)
                            .finish(),
                        );
                        for child in children.into_iter().take(24) {
                            let cid = child.id.clone();
                            col.add_child(
                                Container::new(
                                    EventHandler::new(
                                        ui_text::mono(
                                            format!("→ {}", child.label.chars().take(48).collect::<String>()),
                                            self.font,
                                        )
                                        .with_color(theme::muted())
                                        .finish(),
                                    )
                                    .on_left_mouse_down(move |ctx, _, _| {
                                        ctx.dispatch_typed_action(MemoryAction::SelectNode(
                                            cid.clone(),
                                        ));
                                        DispatchEventResult::StopPropagation
                                    })
                                    .finish(),
                                )
                                .with_margin_top(4.0)
                                .finish(),
                            );
                        }
                    }
                } else {
                    col.add_child(
                        Container::new(
                            ui_text::body(
                                wormhole_i18n::t("settings.memory.graph_select_hint"),
                                self.font,
                            )
                            .with_color(theme::placeholder())
                            .finish(),
                        )
                        .with_margin_top(8.0)
                        .finish(),
                    );
                }
            }
            _ => {
                col.add_child(
                    Container::new(
                        ui_text::body(
                            wormhole_i18n::t("settings.memory.graph_select_hint"),
                            self.font,
                        )
                        .with_color(theme::placeholder())
                        .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                );
            }
        }
        Container::new(col.finish())
            .with_uniform_padding(10.0)
            .with_background(theme::bg())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
            .finish()
    }

    fn browse_pane(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.memory.browse_hint"),
            self.font,
        ));
        let draft = self.browse_query.clone();
        let marked = self.browse_field.marked_text.clone();
        let field = TextFieldInput::builder(
            EventHandler::new(
                Container::new(render_field_with_caret(
                    &draft,
                    &marked,
                    &wormhole_i18n::t("settings.memory.browse_placeholder"),
                    self.font,
                    self.browse_focused,
                    self.busy,
                    true,
                    self.browse_field.cursor,
                ))
                .with_uniform_padding(10.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
            )
            .with_automation_id("settings:memory_browse_query")
            .with_automation_label(wormhole_i18n::t("settings.memory.browse_placeholder"))
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(MemoryAction::FocusBrowseSearch);
                DispatchEventResult::StopPropagation
            })
            .finish(),
            |ctx, action| {
                ctx.dispatch_typed_action(MemoryAction::BrowseSearchEdit(action));
            },
        )
        .focused(self.browse_focused)
        .disabled(self.busy)
        .ime_preedit(!marked.is_empty())
        .finish();
        let mut search_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        search_row.add_child(Expanded::new(1.0, field).finish());
        search_row.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.browse_search"),
                MemoryAction::RunBrowseSearch,
                self.busy || self.browse_query.trim().is_empty(),
                true,
                "settings:memory_browse_search",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(search_row.finish())
                .with_margin_top(10.0)
                .finish(),
        );

        let mut body = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_size(MainAxisSize::Max);
        let mut hits = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        if self.browse_hits.is_empty() {
            hits.add_child(
                ui_text::body(wormhole_i18n::t("settings.memory.browse_empty"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            );
        } else {
            for hit in &self.browse_hits {
                let selected = self.selected_hit_id.as_deref() == Some(hit.id.as_str());
                let id = hit.id.clone();
                hits.add_child(
                    Container::new(
                        EventHandler::new(
                            ui_text::mono(hit.label.clone(), self.font)
                                .with_color(theme::text())
                                .finish(),
                        )
                        .on_left_mouse_down(move |ctx, _, _| {
                            ctx.dispatch_typed_action(MemoryAction::SelectBrowseHit(id.clone()));
                            DispatchEventResult::StopPropagation
                        })
                        .finish(),
                    )
                    .with_margin_top(4.0)
                    .with_uniform_padding(6.0)
                    .with_background(if selected {
                        theme::accent_cool_bg(40)
                    } else {
                        theme::panel_elevated()
                    })
                    .finish(),
                );
            }
        }
        body.add_child(Expanded::new(
            1.0,
            Container::new(hits.finish())
                .with_uniform_padding(10.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                .with_margin_right(10.0)
                .finish(),
        ).finish());
        let mut detail = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        detail.add_child(
            ui_text::body(wormhole_i18n::t("settings.memory.browse_detail"), self.font)
                .with_color(theme::muted())
                .finish(),
        );
        detail.add_child(
            Container::new(
                ui_text::mono(self.hit_detail.clone(), self.font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_top(8.0)
            .finish(),
        );
        detail.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.browse_drill"),
                MemoryAction::DrillSelected,
                self.busy || self.selected_hit_id.is_none(),
                false,
                "settings:memory_browse_drill",
            ))
            .with_margin_top(10.0)
            .finish(),
        );
        body.add_child(
            ConstrainedBox::new(
                Container::new(detail.finish())
                    .with_uniform_padding(10.0)
                    .with_background(theme::bg())
                    .with_border(Border::all(1.0).with_border_fill(theme::border()))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                    .finish(),
            )
            .with_width(320.0)
            .finish(),
        );
        col.add_child(
            EventHandler::new(
                Container::new(body.finish())
                    .with_margin_top(12.0)
                    .finish(),
            )
            .with_automation_id("settings:memory_browse")
            .with_automation_label(wormhole_i18n::t("settings.memory.tab_browse"))
            .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
            .finish(),
        );
        col.finish()
    }

    fn vault_pane(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(section_hint(
            wormhole_i18n::t("settings.memory.hint"),
            self.font,
        ));
        if let Some(status) = &self.status {
            col.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::mono(
                            format!(
                                "{} · md={} · sources={}",
                                status.content_root, status.content_md_files, status.sources_count
                            ),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                )
                .with_automation_id("settings:memory_vault_path")
                .with_automation_label(wormhole_i18n::t("settings.memory.vault_path_label"))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            );
            col.add_child(
                Container::new(
                    ui_text::body(
                        format!(
                            "{}: {}",
                            wormhole_i18n::t("settings.memory.index_label"),
                            status.index_workspace
                        ),
                        self.font,
                    )
                    .with_color(theme::placeholder())
                    .finish(),
                )
                .with_margin_top(6.0)
                .finish(),
            );
            if !status.embeddings_available {
                col.add_child(
                    Container::new(
                        ui_text::body(
                            wormhole_i18n::t("settings.memory.embeddings_unavailable"),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_margin_top(6.0)
                    .finish(),
                );
            }
            if let Some(at) = status.last_reindex_at_ms {
                let summary = status
                    .last_reindex_summary
                    .as_deref()
                    .unwrap_or("");
                let label = format!(
                    "{}: {} · {}",
                    wormhole_i18n::t("settings.memory.pipeline_label"),
                    at,
                    if summary.is_empty() {
                        wormhole_i18n::t("settings.memory.pipeline_ok")
                    } else {
                        summary.chars().take(120).collect::<String>()
                    }
                );
                col.add_child(
                    EventHandler::new(
                        Container::new(
                            ui_text::body(label, self.font)
                                .with_color(theme::placeholder())
                                .finish(),
                        )
                        .with_margin_top(6.0)
                        .finish(),
                    )
                    .with_automation_id("settings:memory_pipeline")
                    .with_automation_label(wormhole_i18n::t("settings.memory.pipeline_label"))
                    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                    .finish(),
                );
            }
        }
        let mut actions = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        actions.add_child(self.action_button(
            &wormhole_i18n::t("settings.memory.open_obsidian"),
            MemoryAction::OpenObsidian,
            self.busy,
            true,
            "settings:memory_vault_obsidian",
        ));
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.reveal_folder"),
                MemoryAction::RevealVault,
                self.busy,
                false,
                "settings:memory_vault_reveal",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.reindex"),
                MemoryAction::Reindex,
                self.busy,
                false,
                "settings:memory_vault_reindex",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.ingest_agent"),
                MemoryAction::IngestAgentTranscripts,
                self.busy,
                false,
                "settings:memory_ingest_agent",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.rss_sync"),
                MemoryAction::SyncRss,
                self.busy,
                false,
                "settings:memory_rss_sync",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.rss_seed_us"),
                MemoryAction::SeedUsMarketRss,
                self.busy,
                false,
                "settings:memory_rss_seed_us",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        actions.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.persona_run"),
                MemoryAction::RunPersona,
                self.busy,
                false,
                "settings:memory_persona_run",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(actions.finish())
                .with_margin_top(12.0)
                .finish(),
        );

        col.add_child(
            Container::new(
                ui_text::body(wormhole_i18n::t("settings.memory.sources_label"), self.font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_margin_top(16.0)
            .finish(),
        );
        if self.sources.is_empty() {
            col.add_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body(
                            wormhole_i18n::t("settings.memory.sources_empty"),
                            self.font,
                        )
                        .with_color(theme::muted())
                        .finish(),
                    )
                    .with_margin_top(8.0)
                    .finish(),
                )
                .with_automation_id("settings:memory_sources")
                .with_automation_label(wormhole_i18n::t("settings.memory.sources_label"))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            );
        } else {
            let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
            for source in &self.sources {
                let enabled = if source.enabled { "on" } else { "off" };
                let kind_label = match source.kind {
                    MemorySourceKind::Rss => "rss",
                    MemorySourceKind::Folder => "folder",
                    MemorySourceKind::Notes => "notes",
                    MemorySourceKind::AgentTranscript => "agent",
                    MemorySourceKind::Composio => "composio",
                };
                let mut row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max);
                row.add_child(Expanded::new(
                    1.0,
                    ui_text::mono(
                        format!("{kind_label}: {} · {} · {enabled}", source.id, source.path),
                        self.font,
                    )
                    .with_color(theme::text())
                    .finish(),
                ).finish());
                row.add_child(
                    Container::new(self.action_button(
                        &wormhole_i18n::t("settings.memory.sources_remove"),
                        MemoryAction::RemoveSource(source.id.clone()),
                        self.busy,
                        false,
                        "settings:memory_source_remove",
                    ))
                    .with_margin_left(8.0)
                    .finish(),
                );
                list.add_child(
                    Container::new(row.finish())
                        .with_margin_top(6.0)
                        .finish(),
                );
            }
            col.add_child(
                EventHandler::new(list.finish())
                    .with_automation_id("settings:memory_sources")
                    .with_automation_label(wormhole_i18n::t("settings.memory.sources_label"))
                    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                    .finish(),
            );
        }

        let draft = self.source_draft.clone();
        let marked = self.source_field.marked_text.clone();
        let field = TextFieldInput::builder(
            EventHandler::new(
                Container::new(render_field_with_caret(
                    &draft,
                    &marked,
                    &wormhole_i18n::t("settings.memory.sources_path_placeholder"),
                    self.font,
                    self.source_focused,
                    self.busy,
                    true,
                    self.source_field.cursor,
                ))
                .with_uniform_padding(10.0)
                .with_background(theme::bg())
                .with_border(Border::all(1.0).with_border_fill(theme::border()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
                .finish(),
            )
            .with_automation_id("settings:memory_source_path")
            .with_automation_label(wormhole_i18n::t("settings.memory.sources_path_label"))
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(MemoryAction::FocusSourcePath);
                DispatchEventResult::StopPropagation
            })
            .finish(),
            |ctx, action| {
                ctx.dispatch_typed_action(MemoryAction::SourcePathEdit(action));
            },
        )
        .focused(self.source_focused)
        .disabled(self.busy)
        .ime_preedit(!marked.is_empty())
        .finish();
        let mut add_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);
        add_row.add_child(Expanded::new(1.0, field).finish());
        add_row.add_child(
            Container::new(self.action_button(
                &wormhole_i18n::t("settings.memory.sources_add"),
                MemoryAction::AddSource,
                self.busy || self.source_draft.trim().is_empty(),
                true,
                "settings:memory_source_add",
            ))
            .with_margin_left(8.0)
            .finish(),
        );
        col.add_child(
            Container::new(add_row.finish())
                .with_margin_top(10.0)
                .finish(),
        );
        col.finish()
    }
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "source" => 0,
        "summary" => 1,
        "chunk" => 2,
        "contact" => 3,
        _ => 9,
    }
}

/// Stable short id for an RSS URL (host + path hash), suitable as MemorySource.id.
fn rss_source_id_from_url(url: &str) -> String {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or("rss")
        .trim()
        .trim_start_matches("www.");
    let host_slug: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut hash: u32 = 2166136261;
    for b in trimmed.bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(16777619);
    }
    format!("rss-{host_slug}-{:08x}", hash)
}

fn format_node_detail(node: &GraphNode) -> String {
    let mut lines = vec![
        format!("id: {}", node.id),
        format!("kind: {}", node.kind),
        format!("label: {}", node.label),
    ];
    if let Some(level) = node.level {
        lines.push(format!("level: {level}"));
    }
    if let Some(parent) = &node.parent_id {
        lines.push(format!("parent: {parent}"));
    }
    if let Some(scope) = &node.tree_scope {
        lines.push(format!("scope: {scope}"));
    }
    if let Some(file) = &node.file_basename {
        lines.push(format!("file: {file}.md"));
    }
    if let Some(count) = node.child_count {
        lines.push(format!("children: {count}"));
    }
    lines.join("\n")
}

fn related_nodes(graph: &GraphExport, id: &str) -> Vec<GraphNode> {
    let mut out = Vec::new();
    for node in &graph.nodes {
        if node.parent_id.as_deref() == Some(id) {
            out.push(node.clone());
        }
    }
    for edge in &graph.edges {
        let other = if edge.from == id {
            Some(edge.to.as_str())
        } else if edge.to == id {
            Some(edge.from.as_str())
        } else {
            None
        };
        if let Some(other_id) = other {
            if let Some(node) = graph.nodes.iter().find(|n| n.id == other_id) {
                if !out.iter().any(|n| n.id == node.id) {
                    out.push(node.clone());
                }
            }
        }
    }
    out
}

fn parse_browse_hits(payload: &serde_json::Value) -> Vec<BrowseHit> {
    let mut hits = Vec::new();
    let arrays = [
        payload.get("hits"),
        payload.get("entities"),
        payload.get("results"),
        payload.as_array().map(|_| payload),
    ];
    for maybe in arrays.into_iter().flatten() {
        if let Some(arr) = maybe.as_array() {
            for item in arr {
                let id = item
                    .get("id")
                    .or_else(|| item.get("entity_id"))
                    .or_else(|| item.get("node_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                let label = item
                    .get("label")
                    .or_else(|| item.get("surface"))
                    .or_else(|| item.get("name"))
                    .or_else(|| item.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(id.as_str())
                    .chars()
                    .take(80)
                    .collect();
                hits.push(BrowseHit { id, label });
            }
        }
    }
    hits
}

impl Entity for MemoryView {
    type Event = ();
}

impl View for MemoryView {
    fn ui_name() -> &'static str {
        "MemoryView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(self.toolbar());
        match self.pane {
            MemoryPane::Graph => col.add_child(self.graph_pane()),
            MemoryPane::Browse => col.add_child(self.browse_pane()),
            MemoryPane::Vault => col.add_child(self.vault_pane()),
        }
        if !self.message.is_empty() {
            col.add_child(
                EventHandler::new(
                    Container::new(status_line(self.message.clone(), self.font, self.tone))
                        .with_margin_top(12.0)
                        .finish(),
                )
                .with_automation_id("settings:memory_status")
                .with_automation_label(wormhole_i18n::t("settings.memory.status_label"))
                .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
                .finish(),
            );
        }
        col.finish()
    }
}

impl TypedActionView for MemoryView {
    type Action = MemoryAction;

    fn handle_action(&mut self, action: &MemoryAction, ctx: &mut ViewContext<Self>) {
        match action {
            MemoryAction::SelectPane(index) => {
                self.pane = match index {
                    1 => MemoryPane::Browse,
                    2 => MemoryPane::Vault,
                    _ => MemoryPane::Graph,
                };
                self.browse_focused = false;
                self.source_focused = false;
                ctx.notify();
            }
            MemoryAction::SetGraphMode(mode) => {
                if self.graph_mode != *mode {
                    self.graph_mode = *mode;
                    self.refresh_graph(ctx);
                }
            }
            MemoryAction::Refresh => self.refresh_all(ctx),
            MemoryAction::RefreshGraph => self.refresh_graph(ctx),
            MemoryAction::OpenObsidian => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_open_obsidian(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(result) => {
                                view.message = result.guidance.unwrap_or_else(|| {
                                    wormhole_i18n::t("settings.memory.opened_obsidian")
                                });
                                view.tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.open_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::RevealVault => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_reveal_folder(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(_) => {
                                view.message = wormhole_i18n::t("settings.memory.revealed");
                                view.tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.reveal_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::Reindex => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.message = wormhole_i18n::t("settings.memory.reindexing");
                self.tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_reindex(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(value) => {
                                view.message = format!(
                                    "{}: {value}",
                                    wormhole_i18n::t("settings.memory.reindex_done")
                                );
                                view.tone = StatusTone::Success;
                                view.refresh_all(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.reindex_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::IngestAgentTranscripts => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_ingest_agent_transcripts(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(summary) => {
                                view.message = format!(
                                    "{}: seen={} written={} ingested={}",
                                    wormhole_i18n::t("settings.memory.ingest_agent_done"),
                                    summary.sessions_seen,
                                    summary.sessions_written,
                                    summary.sessions_ingested
                                );
                                view.tone = StatusTone::Success;
                                view.refresh_all(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.ingest_agent_failed")
                                );
                                view.tone = StatusTone::Danger;
                                ctx.notify();
                            }
                        }
                    },
                );
            }
            MemoryAction::SyncRss => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.message = wormhole_i18n::t("settings.memory.rss_syncing");
                self.tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_rss_sync(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(summary) => {
                                let written: u64 =
                                    summary.feeds.iter().map(|f| f.items_written).sum();
                                let ingested: u64 =
                                    summary.feeds.iter().map(|f| f.items_ingested).sum();
                                let errors: usize =
                                    summary.feeds.iter().map(|f| f.errors.len()).sum();
                                view.message = format!(
                                    "{}: feeds={} written={} ingested={} errors={}",
                                    wormhole_i18n::t("settings.memory.rss_sync_done"),
                                    summary.feeds.len(),
                                    written,
                                    ingested,
                                    errors
                                );
                                view.tone = if errors > 0 {
                                    StatusTone::Danger
                                } else {
                                    StatusTone::Success
                                };
                                view.refresh_all(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.rss_sync_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::SeedUsMarketRss => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_rss_seed_us_markets(&state).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(sources) => {
                                view.message = format!(
                                    "{}: {}",
                                    wormhole_i18n::t("settings.memory.rss_seed_done"),
                                    sources
                                        .iter()
                                        .map(|s| s.id.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                                view.tone = StatusTone::Success;
                                view.refresh_status(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.rss_seed_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::RunPersona => {
                if self.busy {
                    return;
                }
                self.busy = true;
                self.message = wormhole_i18n::t("settings.memory.persona_running");
                self.tone = StatusTone::Placeholder;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        let _ = memory_persona_status(&state).await;
                        memory_persona_run(
                            &state,
                            MemoryPersonaRunParams {
                                backfill: false,
                                max_sessions: 40,
                                identity: None,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(summary) => {
                                view.message = format!(
                                    "{}: processed={} failed={} pack={}",
                                    wormhole_i18n::t("settings.memory.persona_run_done"),
                                    summary.report.sessions_processed,
                                    summary.report.sessions_failed,
                                    summary
                                        .vault_pack_path
                                        .as_deref()
                                        .unwrap_or(summary.pack_path.as_str())
                                );
                                view.tone = if summary.report.sessions_failed > 0 {
                                    StatusTone::Danger
                                } else {
                                    StatusTone::Success
                                };
                                view.refresh_status(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.persona_run_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::SelectNode(id) => {
                self.selected_node_id = Some(id.clone());
                ctx.notify();
            }
            MemoryAction::FocusBrowseSearch => {
                self.browse_focused = true;
                self.source_focused = false;
                ctx.notify();
            }
            MemoryAction::BrowseSearchEdit(action) => {
                self.browse_field.apply(&mut self.browse_query, action);
                ctx.notify();
            }
            MemoryAction::RunBrowseSearch => {
                if self.busy {
                    return;
                }
                let query = self.browse_query.trim().to_string();
                if query.is_empty() {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_tree(
                            &state,
                            MemoryTreeQuery {
                                mode: MemoryTreeMode::SearchEntities,
                                query: Some(query),
                                source_id: None,
                                node_id: None,
                                chunk_ids: None,
                                limit: Some(40),
                                max_depth: None,
                                since_ms: None,
                                until_ms: None,
                                time_window_days: None,
                                title: None,
                                body: None,
                                path: None,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(result) => {
                                view.browse_hits = parse_browse_hits(&result.payload);
                                view.selected_hit_id =
                                    view.browse_hits.first().map(|h| h.id.clone());
                                view.hit_detail = serde_json::to_string_pretty(&result.payload)
                                    .unwrap_or_else(|_| result.payload.to_string());
                                view.message = format!(
                                    "{}: {}",
                                    wormhole_i18n::t("settings.memory.browse_done"),
                                    view.browse_hits.len()
                                );
                                view.tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.browse_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::SelectBrowseHit(id) => {
                self.selected_hit_id = Some(id.clone());
                if let Some(hit) = self.browse_hits.iter().find(|h| h.id == *id) {
                    self.hit_detail = format!("{} · {}", hit.id, hit.label);
                }
                ctx.notify();
            }
            MemoryAction::DrillSelected => {
                let Some(node_id) = self.selected_hit_id.clone() else {
                    return;
                };
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_tree(
                            &state,
                            MemoryTreeQuery {
                                mode: MemoryTreeMode::DrillDown,
                                query: None,
                                source_id: None,
                                node_id: Some(node_id),
                                chunk_ids: None,
                                limit: Some(40),
                                max_depth: Some(2),
                                since_ms: None,
                                until_ms: None,
                                time_window_days: None,
                                title: None,
                                body: None,
                                path: None,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(result) => {
                                view.hit_detail = serde_json::to_string_pretty(&result.payload)
                                    .unwrap_or_else(|_| result.payload.to_string());
                                view.message = wormhole_i18n::t("settings.memory.browse_drilled");
                                view.tone = StatusTone::Success;
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.browse_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::FocusSourcePath => {
                self.source_focused = true;
                self.browse_focused = false;
                ctx.notify();
            }
            MemoryAction::SourcePathEdit(action) => {
                self.source_field.apply(&mut self.source_draft, action);
                ctx.notify();
            }
            MemoryAction::AddSource => {
                let path = self.source_draft.trim().to_string();
                if path.is_empty() || self.busy {
                    return;
                }
                let is_rss = path.starts_with("https://") || path.starts_with("http://");
                let (id, kind) = if is_rss {
                    (rss_source_id_from_url(&path), Some(MemorySourceKind::Rss))
                } else {
                    let id = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("folder")
                        .to_string();
                    (id, Some(MemorySourceKind::Folder))
                };
                self.busy = true;
                let core = self.core.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_sources_upsert(
                            &state,
                            MemorySourcesUpsertParams {
                                id,
                                path,
                                enabled: true,
                                kind,
                            },
                        )
                        .await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(source) => {
                                view.source_draft.clear();
                                view.message = format!(
                                    "{}: {}",
                                    wormhole_i18n::t("settings.memory.sources_added"),
                                    source.id
                                );
                                view.tone = StatusTone::Success;
                                view.refresh_status(ctx);
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.sources_add_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
            MemoryAction::RemoveSource(id) => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let core = self.core.clone();
                let id = id.clone();
                ctx.spawn(
                    async move {
                        let state = core.runtime().state.clone();
                        memory_sources_remove(&state, MemorySourcesRemoveParams { id }).await
                    },
                    |view, output, ctx| {
                        view.busy = false;
                        match output {
                            Ok(true) => {
                                view.message =
                                    wormhole_i18n::t("settings.memory.sources_removed");
                                view.tone = StatusTone::Success;
                                view.refresh_status(ctx);
                            }
                            Ok(false) => {
                                view.message =
                                    wormhole_i18n::t("settings.memory.sources_remove_missing");
                                view.tone = StatusTone::Danger;
                            }
                            Err(error) => {
                                view.message = format!(
                                    "{}: {error}",
                                    wormhole_i18n::t("settings.memory.sources_remove_failed")
                                );
                                view.tone = StatusTone::Danger;
                            }
                        }
                        ctx.notify();
                    },
                );
            }
        }
    }
}
