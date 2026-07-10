//! Agent panel sidebar — 项目树 + 独立对话（`desktop-current.html`）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pathfinder_color::ColorU;
use serde::{Deserialize, Serialize};
use warpui::elements::Fill;
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    agent_row_active_bg, agent_sidebar_bg, agent_sidebar_label, chat_search_pill,
    positioned_context_menu, section_hint, AGENT_ICON_BTN_RADIUS, AGENT_ROW_RADIUS, SECTION_PADDING,
};
use crate::ui::theme;
use crate::ui_text;

pub const SIDEBAR_WIDTH: f32 = 260.0;
const ARCHIVE_FILE: &str = "agent-archived.json";
const ARCHIVED_SESSIONS_FILE: &str = "agent-archived-sessions.json";
const PROJECTS_FILE: &str = "agent-projects.json";
const SIDEBAR_MENU_WIDTH: f32 = 168.0;
const SIDEBAR_MENU_ANCHOR_GAP: f32 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProject {
    pub id: String,
    pub label: String,
    pub time: String,
    pub folder_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: String,
    pub label: String,
    pub project_id: Option<String>,
    pub time: String,
    pub running: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentProjectsFile {
    projects: Vec<AgentProject>,
    expanded: Vec<String>,
}

pub fn is_standalone(session: &AgentSession) -> bool {
    session.project_id.is_none()
}

pub fn session_matches_query(session: &AgentSession, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hay = format!("{} {}", session.label, session.prompt).to_lowercase();
    hay.contains(&q)
}

pub fn project_threads<'a>(
    project_id: &str,
    sessions: &'a [AgentSession],
    archived_ids: &HashSet<String>,
    query: &str,
) -> Vec<&'a AgentSession> {
    sessions
        .iter()
        .filter(|s| {
            s.project_id.as_deref() == Some(project_id)
                && !archived_ids.contains(&s.id)
                && session_matches_query(s, query)
        })
        .collect()
}

pub fn standalone_chats<'a>(
    sessions: &'a [AgentSession],
    archived_ids: &HashSet<String>,
    query: &str,
) -> Vec<&'a AgentSession> {
    sessions
        .iter()
        .filter(|s| is_standalone(s) && !archived_ids.contains(&s.id) && session_matches_query(s, query))
        .collect()
}

pub fn archived_sessions_all<'a>(
    sessions: &'a [AgentSession],
    archived_ids: &HashSet<String>,
    query: &str,
) -> Vec<&'a AgentSession> {
    sessions
        .iter()
        .filter(|s| archived_ids.contains(&s.id) && session_matches_query(s, query))
        .collect()
}

pub fn archived_count_all(sessions: &[AgentSession], archived_ids: &HashSet<String>) -> usize {
    sessions
        .iter()
        .filter(|s| archived_ids.contains(&s.id))
        .count()
}

pub fn load_archived_ids(data_dir: &Path) -> HashSet<String> {
    let path = data_dir.join(ARCHIVE_FILE);
    let raw = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return HashSet::new(),
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .map(|ids| ids.into_iter().collect())
        .unwrap_or_default()
}

pub fn save_archived_ids(data_dir: &Path, ids: &HashSet<String>) {
    let path = data_dir.join(ARCHIVE_FILE);
    let mut list: Vec<&String> = ids.iter().collect();
    list.sort();
    if let Ok(raw) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(path, raw);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedSessionSnapshot {
    pub id: String,
    pub label: String,
    pub project_id: Option<String>,
    pub time: String,
}

pub fn load_archived_snapshots(data_dir: &Path) -> Vec<ArchivedSessionSnapshot> {
    let path = data_dir.join(ARCHIVED_SESSIONS_FILE);
    let raw = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_archived_snapshots(data_dir: &Path, snapshots: &[ArchivedSessionSnapshot]) {
    let path = data_dir.join(ARCHIVED_SESSIONS_FILE);
    if let Ok(raw) = serde_json::to_string_pretty(snapshots) {
        let _ = std::fs::write(path, raw);
    }
}

pub fn upsert_archived_snapshot(data_dir: &Path, snapshot: &ArchivedSessionSnapshot) {
    let mut snapshots = load_archived_snapshots(data_dir);
    if let Some(existing) = snapshots.iter_mut().find(|s| s.id == snapshot.id) {
        *existing = snapshot.clone();
    } else {
        snapshots.push(snapshot.clone());
    }
    snapshots.sort_by(|a, b| a.label.cmp(&b.label));
    save_archived_snapshots(data_dir, &snapshots);
}

pub fn remove_archived_snapshot(data_dir: &Path, session_id: &str) {
    let mut snapshots = load_archived_snapshots(data_dir);
    snapshots.retain(|s| s.id != session_id);
    save_archived_snapshots(data_dir, &snapshots);
}

pub fn archived_snapshot_count(data_dir: &Path) -> usize {
    load_archived_snapshots(data_dir).len()
}

pub fn load_projects_state(data_dir: &Path) -> (Vec<AgentProject>, HashSet<String>) {
    let path = data_dir.join(PROJECTS_FILE);
    let raw = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return (Vec::new(), HashSet::new()),
    };
    match serde_json::from_str::<AgentProjectsFile>(&raw) {
        Ok(file) => (file.projects, file.expanded.into_iter().collect()),
        Err(_) => (Vec::new(), HashSet::new()),
    }
}

pub fn save_projects_state(
    data_dir: &Path,
    projects: &[AgentProject],
    expanded: &HashSet<String>,
) {
    let path = data_dir.join(PROJECTS_FILE);
    let mut expanded_list: Vec<&String> = expanded.iter().collect();
    expanded_list.sort();
    let file = AgentProjectsFile {
        projects: projects.to_vec(),
        expanded: expanded_list.into_iter().cloned().collect(),
    };
    if let Ok(raw) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(path, raw);
    }
}

const SIDEBAR_ICON_BTN: f32 = 26.0;
const PROJECT_HEAD_ICON_BTN: f32 = 24.0;

fn agent_sidebar_icon_btn(
    hover_key: &'static str,
    sidebar_hover: Option<&str>,
    action: AgentPanelAction,
    icon_path: &'static str,
    btn_size: f32,
    icon_size: f32,
) -> Box<dyn Element> {
    let hovered = sidebar_hover == Some(hover_key);
    let icon_color = if hovered {
        theme::accent_cool()
    } else {
        theme::muted()
    };
    let mut container = Container::new(
        ConstrainedBox::new(Align::new(icons::icon(icon_path, icon_size, icon_color)).finish())
            .with_width(btn_size)
            .with_height(btn_size)
            .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
        AGENT_ICON_BTN_RADIUS,
    )));
    if hovered {
        container = container.with_background(theme::accent_cool_bg(20));
    }
    let hover_key_in = hover_key.to_string();
    let hover_key_out = hover_key.to_string();
    EventHandler::new(container.finish())
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SetSidebarHover(Some(
                    hover_key_in.clone(),
                )));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(
                hover_key_out.clone(),
            ));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn agent_sidebar_icon_btn_dynamic(
    hover_key: String,
    sidebar_hover: Option<&str>,
    action: AgentPanelAction,
    icon_path: &'static str,
    btn_size: f32,
    icon_size: f32,
) -> Box<dyn Element> {
    let hovered = sidebar_hover == Some(hover_key.as_str());
    let icon_color = if hovered {
        theme::accent_cool()
    } else {
        theme::muted()
    };
    let mut container = Container::new(
        ConstrainedBox::new(Align::new(icons::icon(icon_path, icon_size, icon_color)).finish())
            .with_width(btn_size)
            .with_height(btn_size)
            .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
        AGENT_ICON_BTN_RADIUS,
    )));
    if hovered {
        container = container.with_background(theme::accent_cool_bg(20));
    }
    let hover_key_in = hover_key.clone();
    let hover_key_out = hover_key;
    EventHandler::new(container.finish())
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SetSidebarHover(Some(
                    hover_key_in.clone(),
                )));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(
                hover_key_out.clone(),
            ));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

/// Section header matching `.agent-sidebar-head` (`space-between`, actions `gap: 2px`).
///
/// Pass `more` as `Some` for chats (`⋯` + new); `None` for projects (new only, HTML).
fn sidebar_section_head(
    font: FamilyId,
    label: &str,
    more: Option<(&'static str, AgentPanelAction)>,
    new_hover_key: &'static str,
    new_action: AgentPanelAction,
    new_icon: &'static str,
    menu_open: bool,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let new_btn = agent_sidebar_icon_btn(
        new_hover_key,
        sidebar_hover,
        new_action,
        new_icon,
        SIDEBAR_ICON_BTN,
        icons::AGENT_ICON_SIZE,
    );

    let mut actions = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    if let Some((more_hover_key, more_action)) = more {
        actions = actions.with_child(
            Container::new(agent_sidebar_icon_btn(
                more_hover_key,
                sidebar_hover,
                more_action,
                "agent-more.svg",
                SIDEBAR_ICON_BTN,
                icons::AGENT_ICON_SIZE,
            ))
            .with_margin_right(2.0)
            .finish(),
        );
    }
    actions = actions.with_child(new_btn);
    let actions_row = actions.finish();

    let mut actions_stack = Stack::new();
    actions_stack.add_child(actions_row);
    if menu_open {
        actions_stack.add_child(
            Align::new(
                Container::new(render_sidebar_empty_menu())
                    .with_margin_top(SIDEBAR_ICON_BTN + SIDEBAR_MENU_ANCHOR_GAP)
                    .finish(),
            )
            .top_right()
            .finish(),
        );
    }

    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(Expanded::new(1.0, agent_sidebar_label(label.to_string(), font)).finish())
        .with_child(actions_stack.finish())
        .finish()
}

fn render_sidebar_empty_menu() -> Box<dyn Element> {
    let panel = Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_min_width(SIDEBAR_MENU_WIDTH)
            .with_width(SIDEBAR_MENU_WIDTH)
            .with_min_height(32.0)
            .finish(),
    )
    .with_uniform_padding(5.0)
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
    .finish();
    EventHandler::new(panel)
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

fn search_box(
    font: FamilyId,
    search: &str,
    search_focused: bool,
) -> Box<dyn Element> {
    let search_border = if search_focused {
        theme::accent_cool()
    } else {
        theme::border()
    };
    let search_text = if search.is_empty() {
        "搜索对话…".to_string()
    } else {
        search.to_string()
    };
    let search_color = if search.is_empty() && !search_focused {
        theme::placeholder()
    } else {
        theme::text()
    };
    let row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Container::new(icons::agent_icon("agent-search.svg", theme::muted()))
                .with_horizontal_margin(4.0)
                .finish(),
        )
        .with_child(
            Expanded::new(
                1.0,
                EventHandler::new(
                    ui_text::body(search_text, font)
                        .with_color(search_color)
                        .finish(),
                )
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(AgentPanelAction::FocusSidebarSearch);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .finish(),
        )
        .finish();

    let pill = chat_search_pill(
        row,
        theme::canvas(),
        search_border,
        7.0,
        10.0,
        AGENT_ROW_RADIUS,
    );

    Container::new(
        ConstrainedBox::new(pill)
            .with_width(SIDEBAR_WIDTH - 4.0)
            .finish(),
    )
    .with_horizontal_margin(2.0)
    .with_margin_bottom(8.0)
    .finish()
}

fn chat_spinner() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_width(11.0)
            .with_height(11.0)
            .finish(),
    )
    .with_border(Border::all(1.5).with_border_color(theme::accent_cool_bg(90)))
    .with_border(Border::top(1.5).with_border_color(theme::accent_cool()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(999.0)))
    .with_horizontal_margin(4.0)
    .finish()
}

fn row_delete_button(
    font: FamilyId,
    action: AgentPanelAction,
    enabled: bool,
    danger: bool,
) -> Box<dyn Element> {
    if !enabled {
        return ConstrainedBox::new(Flex::row().finish())
            .with_width(26.0)
            .with_height(26.0)
            .finish();
    }
    let color = if danger {
        theme::danger()
    } else {
        theme::muted()
    };
    let btn = Container::new(
        ConstrainedBox::new(
            Align::new(
                ui_text::body("×".to_string(), font)
                    .with_color(color)
                    .finish(),
            )
            .finish(),
        )
        .with_width(26.0)
        .with_height(26.0)
        .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ICON_BTN_RADIUS)))
    .finish();

    EventHandler::new(btn)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn row_item_inner(
    font: FamilyId,
    label: String,
    time: String,
    active: bool,
    show_spinner: bool,
    archived: bool,
    nested: bool,
) -> Box<dyn Element> {
    let fg = if archived {
        theme::muted()
    } else {
        theme::text()
    };
    let bg = if active {
        agent_row_active_bg()
    } else {
        ColorU::transparent_black()
    };
    let time_color = if active {
        theme::accent_cool()
    } else {
        theme::muted()
    };
    let left_pad = if nested { 18.0 } else { 10.0 };
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max);
    if show_spinner {
        row.add_child(chat_spinner());
    }
    row.add_child(
        Shrinkable::new(1.0, ui_text::body(label, font).with_color(fg).finish()).finish(),
    );
    row.add_child(
        Container::new(ui_text::mono(time, font).with_color(time_color).finish())
            .with_horizontal_margin(6.0)
            .finish(),
    );
    Container::new(row.finish())
        .with_padding_left(left_pad)
        .with_padding_right(34.0)
        .with_padding_top(7.0)
        .with_padding_bottom(7.0)
        .with_background(bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
        .finish()
}

fn row_with_delete(
    font: FamilyId,
    label: String,
    time: String,
    active: bool,
    show_spinner: bool,
    archived: bool,
    nested: bool,
    select_action: AgentPanelAction,
    delete_action: AgentPanelAction,
    delete_enabled: bool,
    session_id: String,
) -> Box<dyn Element> {
    let inner = row_item_inner(
        font, label, time, active, show_spinner, archived, nested,
    );
    let select = select_action.clone();
    let main = EventHandler::new(inner)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(select.clone());
            DispatchEventResult::StopPropagation
        })
        .on_right_mouse_down(move |ctx, _, position| {
            ctx.dispatch_typed_action(AgentPanelAction::OpenSessionContextMenu {
                id: session_id.clone(),
                archived,
                x: position.x(),
                y: position.y(),
            });
            DispatchEventResult::StopPropagation
        })
        .finish();

    Stack::new()
        .with_child(main)
        .with_child(
            Align::new(row_delete_button(font, delete_action, delete_enabled, true)).right().finish(),
        )
        .finish()
}

fn project_row_more_btn(
    font: FamilyId,
    hover_key: String,
    sidebar_hover: Option<&str>,
    project_id: String,
    menu_open: bool,
    can_delete: bool,
) -> Box<dyn Element> {
    let hovered = sidebar_hover == Some(hover_key.as_str());
    let icon_color = if hovered {
        theme::accent_cool()
    } else {
        theme::muted()
    };
    let mut container = Container::new(
        ConstrainedBox::new(
            Align::new(icons::icon(
                "agent-more.svg",
                icons::AGENT_ICON_SIZE,
                icon_color,
            ))
            .finish(),
        )
        .with_width(PROJECT_HEAD_ICON_BTN)
        .with_height(PROJECT_HEAD_ICON_BTN)
        .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
        AGENT_ICON_BTN_RADIUS,
    )));
    if hovered {
        container = container.with_background(theme::accent_cool_bg(20));
    }
    let hover_key_in = hover_key.clone();
    let hover_key_out = hover_key;
    let open_id = project_id.clone();
    let btn = EventHandler::new(container.finish())
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SetSidebarHover(Some(
                    hover_key_in.clone(),
                )));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(
                hover_key_out.clone(),
            ));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::OpenProjectRowMenu(open_id.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish();

    let mut stack = Stack::new();
    stack.add_child(btn);
    if menu_open {
        stack.add_child(
            Align::new(
                Container::new(project_row_menu_panel(font, &project_id, can_delete))
                    .with_margin_top(PROJECT_HEAD_ICON_BTN + SIDEBAR_MENU_ANCHOR_GAP)
                    .finish(),
            )
            .top_left()
            .finish(),
        );
    }
    stack.finish()
}

fn project_tree_block(
    font: FamilyId,
    project: &AgentProject,
    sessions: &[AgentSession],
    active_project_id: &str,
    active_session_id: &str,
    expanded: bool,
    search: &str,
    archived_ids: &HashSet<String>,
    can_delete_project: bool,
    project_row_menu: Option<&str>,
    sidebar_hover: Option<&str>,
    project_head_hover: Option<&str>,
) -> Box<dyn Element> {
    let chats = project_threads(&project.id, sessions, archived_ids, search);
    let force_expanded = expanded || (!search.trim().is_empty() && !chats.is_empty());
    let head_selected = project.id == active_project_id;

    let head_bg = if head_selected {
        agent_row_active_bg()
    } else {
        ColorU::transparent_black()
    };

    let chevron_path = if force_expanded {
        "agent-chevron-down.svg"
    } else {
        "agent-chevron.svg"
    };
    let toggle_hovered = project_head_hover == Some(project.id.as_str());
    let toggle_color = if toggle_hovered {
        theme::accent_cool()
    } else {
        theme::muted()
    };
    let toggle_id = project.id.clone();
    let toggle = EventHandler::new(
        Container::new(
            ConstrainedBox::new(
                Align::new(icons::icon(chevron_path, 12.0, toggle_color)).finish(),
            )
            .with_width(22.0)
            .with_height(28.0)
            .finish(),
        )
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::ToggleProjectExpanded(toggle_id.clone()));
        DispatchEventResult::StopPropagation
    })
    .finish();

    let select_id = project.id.clone();
    let head_btn = EventHandler::new(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(icons::agent_icon("agent-folder.svg", theme::accent_cool()))
                        .with_horizontal_margin(4.0)
                        .finish(),
                )
                .with_child(
                    Shrinkable::new(
                        1.0,
                        ui_text::body(project.label.clone(), font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_padding_left(4.0)
        .with_padding_right(60.0)
        .with_padding_top(7.0)
        .with_padding_bottom(7.0)
        .with_background(head_bg)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::SelectProject(select_id.clone()));
        DispatchEventResult::StopPropagation
    })
    .finish();

    let new_thread_id = project.id.clone();
    let menu_open = project_row_menu == Some(project.id.as_str());
    let show_head_actions =
        head_selected || project_head_hover == Some(project.id.as_str()) || menu_open;
    let menu_hover_key = format!("project-menu:{}", project.id);
    let new_thread_hover_key = format!("project-new-thread:{}", project.id);
    let head_actions = if show_head_actions {
        Align::new(
            Flex::row()
                .with_child(project_row_more_btn(
                    font,
                    menu_hover_key,
                    sidebar_hover,
                    project.id.clone(),
                    menu_open,
                    can_delete_project,
                ))
                .with_child(agent_sidebar_icon_btn_dynamic(
                    new_thread_hover_key,
                    sidebar_hover,
                    AgentPanelAction::NewProjectThread(new_thread_id),
                    "agent-edit.svg",
                    PROJECT_HEAD_ICON_BTN,
                    icons::AGENT_ICON_SIZE,
                ))
                .finish(),
        )
        .right()
        .finish()
    } else {
        Align::new(ConstrainedBox::new(Flex::row().finish()).with_width(52.0).finish())
            .right()
            .finish()
    };

    let head_hover_id = project.id.clone();
    let head = EventHandler::new(
        Stack::new()
            .with_child(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(toggle)
                    .with_child(Expanded::new(1.0, head_btn).finish())
                    .finish(),
            )
            .with_child(head_actions)
            .finish(),
    )
    .on_mouse_in(
        move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::SetProjectHeadHover(Some(
                head_hover_id.clone(),
            )));
            DispatchEventResult::PropagateToParent
        },
        None,
    )
    .on_mouse_out(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::SetProjectHeadHover(None));
        DispatchEventResult::PropagateToParent
    })
    .finish();

    let mut children_col = Flex::column();
    if force_expanded {
        if chats.is_empty() {
            let hint = if search.trim().is_empty() {
                "暂无对话"
            } else {
                "无匹配对话"
            };
            children_col.add_child(
                Container::new(section_hint(hint, font))
                    .with_padding_left(28.0)
                    .with_padding_top(2.0)
                    .with_padding_bottom(2.0)
                    .finish(),
            );
        } else {
            for session in chats {
                let active = session.id == active_session_id;
                let show_spinner = session.running && active;
                let session_id = session.id.clone();
                children_col.add_child(
                    Container::new(row_with_delete(
                        font,
                        session.label.clone(),
                        session.time.clone(),
                        active,
                        show_spinner,
                        false,
                        true,
                        AgentPanelAction::SelectSession(session.id.clone()),
                        AgentPanelAction::ArchiveSession(session.id.clone()),
                        true,
                        session_id,
                    ))
                    .with_horizontal_padding(2.0)
                    .finish(),
                );
            }
        }
    }

    Flex::column()
        .with_child(Container::new(head).with_horizontal_padding(2.0).finish())
        .with_child(children_col.finish())
        .finish()
}

fn sidebar_menu_item(
    font: FamilyId,
    label: &str,
    disabled: bool,
    danger: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let color = if disabled {
        theme::placeholder()
    } else if danger {
        theme::danger()
    } else {
        theme::text()
    };
    let action_for_click = action.clone();
    let row = Container::new(
        ui_text::body(label.to_string(), font)
            .with_color(color)
            .finish(),
    )
    .with_padding_left(12.0)
    .with_padding_right(12.0)
    .with_padding_top(8.0)
    .with_padding_bottom(8.0)
    .finish();

    if disabled {
        return row;
    }

    EventHandler::new(row)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action_for_click.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn sidebar_dropdown_menu(
    font: FamilyId,
    items: Vec<(&str, bool, bool, AgentPanelAction)>,
) -> Box<dyn Element> {
    let mut col = Flex::column().with_main_axis_size(MainAxisSize::Min);
    for (label, disabled, danger, action) in items {
        col.add_child(sidebar_menu_item(font, label, disabled, danger, action));
    }
    Container::new(
        ConstrainedBox::new(col.finish())
            .with_min_width(SIDEBAR_MENU_WIDTH)
            .with_width(SIDEBAR_MENU_WIDTH)
            .finish(),
    )
    .with_uniform_padding(4.0)
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
    .finish()
}

pub fn render_session_context_menu(
    font: FamilyId,
    session_id: &str,
    archived: bool,
    x: f32,
    y: f32,
) -> Box<dyn Element> {
    let id = session_id.to_string();
    let mut items: Vec<(&str, bool, bool, AgentPanelAction)> = Vec::new();
    if archived {
        items.push((
            "恢复至对话",
            false,
            false,
            AgentPanelAction::RestoreSession(id.clone()),
        ));
        items.push((
            "永久删除",
            false,
            true,
            AgentPanelAction::DeleteSession(id.clone()),
        ));
    } else {
        items.push((
            "归档",
            false,
            false,
            AgentPanelAction::ArchiveSession(id.clone()),
        ));
    }
    positioned_context_menu(x, y, sidebar_dropdown_menu(font, items))
}

/// Project-row ⋯ dropdown panel — HTML `#agent-projects-menu` (anchored in-tree).
fn project_row_menu_panel(font: FamilyId, project_id: &str, can_delete: bool) -> Box<dyn Element> {
    let items = vec![(
        "删除项目",
        !can_delete,
        true,
        AgentPanelAction::OpenProjectDeleteModal(project_id.to_string()),
    )];
    EventHandler::new(sidebar_dropdown_menu(font, items))
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

pub fn render_sidebar(
    font: FamilyId,
    scroll: ClippedScrollStateHandle,
    projects: &[AgentProject],
    sessions: &[AgentSession],
    expanded_project_ids: &HashSet<String>,
    active_project_id: &str,
    active_session_id: &str,
    search: &str,
    search_focused: bool,
    archived_ids: &HashSet<String>,
    chats_menu_open: bool,
    project_row_menu: Option<&str>,
    sidebar_hover: Option<&str>,
    project_head_hover: Option<&str>,
) -> Box<dyn Element> {
    let mut scroll_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    scroll_col.add_child(search_box(font, search, search_focused));

    scroll_col.add_child(
        Container::new(sidebar_section_head(
            font,
            "项目",
            None,
            "new-project",
            AgentPanelAction::OpenProjectCreateModal,
            "agent-edit.svg",
            false,
            sidebar_hover,
        ))
        .with_horizontal_padding(6.0)
        .with_padding_bottom(8.0)
        .finish(),
    );

    let can_delete_project = projects.len() > 1;
    if projects.is_empty() {
        scroll_col.add_child(
            Container::new(section_hint("暂无项目，点击新建", font))
                .with_horizontal_padding(10.0)
                .with_padding_bottom(4.0)
                .finish(),
        );
    } else {
        for project in projects {
            let expanded = expanded_project_ids.contains(&project.id);
            scroll_col.add_child(
                Container::new(project_tree_block(
                    font,
                    project,
                    sessions,
                    active_project_id,
                    active_session_id,
                    expanded,
                    search,
                    archived_ids,
                    can_delete_project,
                    project_row_menu,
                    sidebar_hover,
                    project_head_hover,
                ))
                .with_horizontal_padding(2.0)
                .with_padding_bottom(2.0)
                .finish(),
            );
        }
    }

    scroll_col.add_child(
        Container::new(sidebar_section_head(
            font,
            "对话",
            Some(("chats-more", AgentPanelAction::ToggleChatsMenu)),
            "new-chat",
            AgentPanelAction::NewStandaloneChat,
            "agent-edit.svg",
            chats_menu_open,
            sidebar_hover,
        ))
        .with_horizontal_padding(6.0)
        .with_padding_top(12.0)
        .with_padding_bottom(8.0)
        .finish(),
    );

    let standalone = standalone_chats(sessions, archived_ids, search);
    if standalone.is_empty() {
        let empty = if search.trim().is_empty() {
            "暂无独立对话"
        } else {
            "无匹配独立对话"
        };
        scroll_col.add_child(
            Container::new(section_hint(empty, font))
                .with_horizontal_padding(10.0)
                .finish(),
        );
    } else {
        for session in standalone {
            let active = session.id == active_session_id;
            let show_spinner = session.running && active;
            let session_id = session.id.clone();
            scroll_col.add_child(
                Container::new(row_with_delete(
                    font,
                    session.label.clone(),
                    session.time.clone(),
                    active,
                    show_spinner,
                    false,
                    false,
                    AgentPanelAction::SelectSession(session.id.clone()),
                    AgentPanelAction::ArchiveSession(session.id.clone()),
                    true,
                    session_id,
                ))
                .with_horizontal_padding(2.0)
                .finish(),
            );
        }
    }

    let scroll_body = Container::new(scroll_col.finish())
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(SECTION_PADDING)
        .with_padding_bottom(16.0)
        .finish();

    Container::new(
        Flex::column()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(
                Expanded::new(
                    1.0,
                    ClippedScrollable::vertical(
                        scroll,
                        scroll_body,
                        ScrollbarWidth::Auto,
                        Fill::None,
                        Fill::None,
                        Fill::None,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
    )
    .with_background(agent_sidebar_bg())
    .with_border(Border::right(1.0).with_border_fill(theme::border()))
    .finish()
}

#[cfg(test)]
mod tests {
    use super::{
        archived_count_all, is_standalone, load_archived_ids, load_archived_snapshots,
        load_projects_state, project_threads, remove_archived_snapshot, save_archived_ids,
        save_projects_state, standalone_chats, upsert_archived_snapshot, AgentProject,
        AgentSession, ArchivedSessionSnapshot,
    };
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn archived_ids_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("wormhole-agent-archive-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut ids = HashSet::new();
        ids.insert("session-a".into());
        save_archived_ids(&dir, &ids);
        let loaded = load_archived_ids(&dir);
        assert!(loaded.contains("session-a"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn projects_state_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wormhole-agent-projects-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let projects = vec![AgentProject {
            id: "p1".into(),
            label: "Test".into(),
            time: "刚刚".into(),
            folder_path: PathBuf::from("D:\\Projects\\test"),
        }];
        let mut expanded = HashSet::new();
        expanded.insert("p1".into());
        save_projects_state(&dir, &projects, &expanded);
        let (loaded, loaded_expanded) = load_projects_state(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].label, "Test");
        assert!(loaded_expanded.contains("p1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn standalone_vs_project_threads() {
        let sessions = vec![
            AgentSession {
                id: "a".into(),
                label: "a".into(),
                project_id: Some("p1".into()),
                time: "now".into(),
                running: false,
                prompt: String::new(),
            },
            AgentSession {
                id: "b".into(),
                label: "b".into(),
                project_id: None,
                time: "now".into(),
                running: false,
                prompt: String::new(),
            },
        ];
        let archived = HashSet::new();
        assert_eq!(project_threads("p1", &sessions, &archived, "").len(), 1);
        assert_eq!(standalone_chats(&sessions, &archived, "").len(), 1);
        assert!(is_standalone(&sessions[1]));
        assert!(!is_standalone(&sessions[0]));
    }

    #[test]
    fn archived_count_all_includes_all_projects() {
        let sessions = vec![
            AgentSession {
                id: "a".into(),
                label: "a".into(),
                project_id: Some("p1".into()),
                time: "now".into(),
                running: false,
                prompt: String::new(),
            },
            AgentSession {
                id: "b".into(),
                label: "b".into(),
                project_id: None,
                time: "now".into(),
                running: false,
                prompt: String::new(),
            },
        ];
        let mut archived = HashSet::new();
        archived.insert("a".into());
        archived.insert("b".into());
        assert_eq!(archived_count_all(&sessions, &archived), 2);
    }

    #[test]
    fn archived_snapshot_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "wormhole-archived-snapshots-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let snapshot = ArchivedSessionSnapshot {
            id: "s1".into(),
            label: "测试会话".into(),
            project_id: Some("p1".into()),
            time: "刚刚".into(),
        };
        upsert_archived_snapshot(&dir, &snapshot);
        let loaded = load_archived_snapshots(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "s1");
        remove_archived_snapshot(&dir, "s1");
        assert!(load_archived_snapshots(&dir).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
