//! Agent panel sidebar — 项目树 + 独立对话（`desktop-current.html`）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use serde::{Deserialize, Serialize};
use warpui::elements::Fill;
use warpui::elements::{
    Align, Border, ChildAnchor, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox,
    Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex,
    MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    ScrollbarWidth, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    agent_row_active_bg, agent_sidebar_bg, agent_sidebar_label, chat_search_pill,
    positioned_context_menu, section_hint, AGENT_ICON_BTN_RADIUS, AGENT_ROW_RADIUS,
    SECTION_PADDING,
};
use crate::ui::theme;
use crate::ui_text;

pub const SIDEBAR_WIDTH: f32 = 260.0;
const ARCHIVE_FILE: &str = "agent-archived.json";
const ARCHIVED_SESSIONS_FILE: &str = "agent-archived-sessions.json";
const PROJECTS_FILE: &str = "agent-projects.json";
const SIDEBAR_MENU_MIN_WIDTH: f32 = 120.0;
const SIDEBAR_MENU_MAX_WIDTH: f32 = 180.0;
const CHATS_MENU_WIDTH: f32 = 200.0;
const CHATS_FLYOUT_WIDTH: f32 = 180.0;
const SIDEBAR_MENU_ANCHOR_GAP: f32 = 4.0;

/// Standalone-chat list sort — `#agent-sort-flyout` in `desktop-current.html`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatsSort {
    Created,
    #[default]
    Updated,
}

/// Open submenu in `#agent-chats-menu`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatsFlyout {
    Organize,
    Sort,
}

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
    standalone_chats_sorted(sessions, archived_ids, query, ChatsSort::Updated)
}

pub fn standalone_chats_sorted<'a>(
    sessions: &'a [AgentSession],
    archived_ids: &HashSet<String>,
    query: &str,
    sort: ChatsSort,
) -> Vec<&'a AgentSession> {
    let mut out: Vec<&'a AgentSession> = sessions
        .iter()
        .filter(|s| {
            is_standalone(s) && !archived_ids.contains(&s.id) && session_matches_query(s, query)
        })
        .collect();
    match sort {
        ChatsSort::Updated => {
            // `time` is a relative display string; keep insertion / list order as "recent".
        }
        ChatsSort::Created => {
            out.reverse();
        }
    }
    out
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

pub fn save_projects_state(data_dir: &Path, projects: &[AgentProject], expanded: &HashSet<String>) {
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
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(hover_key_out.clone()));
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
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(hover_key_out.clone()));
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
/// Menus are `add_positioned_overlay_child` (HTML `position: fixed`) so they do not expand row height.
fn sidebar_section_head(
    font: FamilyId,
    label: &str,
    more: Option<(&'static str, AgentPanelAction)>,
    new_hover_key: &'static str,
    new_action: AgentPanelAction,
    new_icon: &'static str,
    menu: Option<Box<dyn Element>>,
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
    if let Some(menu_el) = menu {
        actions_stack.add_positioned_overlay_child(
            menu_el,
            OffsetPositioning::offset_from_parent(
                vec2f(0.0, SIDEBAR_ICON_BTN + SIDEBAR_MENU_ANCHOR_GAP),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
    }

    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(Expanded::new(1.0, agent_sidebar_label(label.to_string(), font)).finish())
        .with_child(actions_stack.finish())
        .finish()
}

fn render_projects_section_menu(
    font: FamilyId,
    project_id: &str,
    can_delete: bool,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let id = project_id.to_string();
    let items = vec![
        (
            "重命名",
            false,
            false,
            AgentPanelAction::RenameProject(id.clone()),
        ),
        (
            "删除项目",
            !can_delete,
            true,
            AgentPanelAction::OpenProjectDeleteModal(id),
        ),
    ];
    EventHandler::new(sidebar_dropdown_menu(font, items, sidebar_hover))
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

fn render_chats_section_menu(
    font: FamilyId,
    sort: ChatsSort,
    flyout: Option<ChatsFlyout>,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(chats_menu_item(
        font,
        "归档所有聊天",
        Some("agent-menu-archive.svg"),
        "chats-archive-all",
        sidebar_hover,
        false,
        false,
        false,
        None,
        AgentPanelAction::ArchiveAllStandaloneChats,
    ));
    col.add_child(menu_separator());
    col.add_child(chats_flyout_parent(
        font,
        "整理侧边栏",
        "agent-menu-organize.svg",
        "chats-organize",
        sidebar_hover,
        flyout == Some(ChatsFlyout::Organize),
        ChatsFlyout::Organize,
        render_organize_flyout(font, sidebar_hover),
    ));
    col.add_child(chats_flyout_parent(
        font,
        "排序条件",
        "agent-menu-sort.svg",
        "chats-sort",
        sidebar_hover,
        flyout == Some(ChatsFlyout::Sort),
        ChatsFlyout::Sort,
        render_sort_flyout(font, sort, sidebar_hover),
    ));

    EventHandler::new(menu_shell(CHATS_MENU_WIDTH, 6.0, 12.0, col.finish()))
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

#[allow(clippy::too_many_arguments)]
fn chats_menu_item(
    font: FamilyId,
    label: &str,
    icon_path: Option<&'static str>,
    hover_key: &'static str,
    sidebar_hover: Option<&str>,
    active: bool,
    has_flyout: bool,
    checked: bool,
    flyout: Option<ChatsFlyout>,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let highlighted = active || sidebar_hover == Some(hover_key);
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);
    if let Some(path) = icon_path {
        row.add_child(icons::icon(
            path,
            icons::AGENT_MENU_ICON_SIZE,
            theme::muted(),
        ));
    }
    row.add_child(
        Expanded::new(
            1.0,
            Container::new(
                ui_text::body(label.to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_margin_left(if icon_path.is_some() { 10.0 } else { 0.0 })
            .finish(),
        )
        .finish(),
    );
    if has_flyout {
        row.add_child(ui_text::body("›", font).with_color(theme::muted()).finish());
    } else if checked {
        row.add_child(
            ui_text::body("✓", font)
                .with_color(theme::accent_cool())
                .finish(),
        );
    }

    let mut container = Container::new(row.finish())
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(9.0)
        .with_padding_bottom(9.0)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)));
    if highlighted {
        container = container.with_background(theme::accent_cool_bg(20));
    }

    EventHandler::new(container.finish())
        .on_mouse_in(
            move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SetSidebarHover(Some(
                    hover_key.to_string(),
                )));
                ctx.dispatch_typed_action(AgentPanelAction::SetChatsFlyout(flyout));
                DispatchEventResult::PropagateToParent
            },
            None,
        )
        .on_mouse_out(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(hover_key.to_string()));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

#[allow(clippy::too_many_arguments)]
fn chats_flyout_parent(
    font: FamilyId,
    label: &str,
    icon_path: &'static str,
    hover_key: &'static str,
    sidebar_hover: Option<&str>,
    open: bool,
    flyout: ChatsFlyout,
    flyout_panel: Box<dyn Element>,
) -> Box<dyn Element> {
    let mut stack = Stack::new();
    stack.add_child(chats_menu_item(
        font,
        label,
        Some(icon_path),
        hover_key,
        sidebar_hover,
        open,
        true,
        false,
        Some(flyout),
        AgentPanelAction::SetChatsFlyout(Some(flyout)),
    ));
    if open {
        stack.add_positioned_overlay_child(
            flyout_panel,
            OffsetPositioning::offset_from_parent(
                vec2f(SIDEBAR_MENU_ANCHOR_GAP, 0.0),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::TopRight,
                ChildAnchor::TopLeft,
            ),
        );
    }
    stack.finish()
}

fn render_organize_flyout(font: FamilyId, sidebar_hover: Option<&str>) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(chats_menu_item(
        font,
        "折叠所有项目",
        None,
        "chats-collapse-all",
        sidebar_hover,
        false,
        false,
        false,
        Some(ChatsFlyout::Organize),
        AgentPanelAction::CollapseAllProjects,
    ));
    col.add_child(chats_menu_item(
        font,
        "展开所有项目",
        None,
        "chats-expand-all",
        sidebar_hover,
        false,
        false,
        false,
        Some(ChatsFlyout::Organize),
        AgentPanelAction::ExpandAllProjects,
    ));
    flyout_shell(col.finish())
}

fn render_sort_flyout(
    font: FamilyId,
    sort: ChatsSort,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(chats_menu_item(
        font,
        "创建时间",
        Some("agent-menu-created.svg"),
        "chats-sort-created",
        sidebar_hover,
        false,
        false,
        sort == ChatsSort::Created,
        Some(ChatsFlyout::Sort),
        AgentPanelAction::SetChatsSort(ChatsSort::Created),
    ));
    col.add_child(chats_menu_item(
        font,
        "最近更新",
        Some("agent-menu-updated.svg"),
        "chats-sort-updated",
        sidebar_hover,
        false,
        false,
        sort == ChatsSort::Updated,
        Some(ChatsFlyout::Sort),
        AgentPanelAction::SetChatsSort(ChatsSort::Updated),
    ));
    flyout_shell(col.finish())
}

fn flyout_shell(child: Box<dyn Element>) -> Box<dyn Element> {
    EventHandler::new(menu_shell(CHATS_FLYOUT_WIDTH, 6.0, 12.0, child))
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish()
}

fn menu_shell(width: f32, padding: f32, radius: f32, child: Box<dyn Element>) -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(child)
            .with_min_width(width)
            .with_width(width)
            .finish(),
    )
    .with_uniform_padding(padding)
    .with_background(theme::panel_elevated())
    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(radius)))
    .finish()
}

fn menu_separator() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_height(1.0)
            .finish(),
    )
    .with_background(theme::border())
    .with_vertical_margin(4.0)
    .with_horizontal_margin(8.0)
    .finish()
}

fn search_box(font: FamilyId, search: &str, search_focused: bool) -> Box<dyn Element> {
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
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
        AGENT_ICON_BTN_RADIUS,
    )))
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
    let inner = row_item_inner(font, label, time, active, show_spinner, archived, nested);
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
            Align::new(row_delete_button(font, delete_action, delete_enabled, true))
                .right()
                .finish(),
        )
        .finish()
}

fn project_row_more_btn(
    font: FamilyId,
    hover_key: String,
    sidebar_hover: Option<&str>,
    project_id: String,
    menu_open: bool,
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
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(hover_key_out.clone()));
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
        // Overlay: does not contribute to Stack size (HTML `position: fixed`).
        stack.add_positioned_overlay_child(
            project_row_menu_panel(font, &project_id, sidebar_hover),
            OffsetPositioning::offset_from_parent(
                vec2f(0.0, PROJECT_HEAD_ICON_BTN + SIDEBAR_MENU_ANCHOR_GAP),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
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
            ConstrainedBox::new(Align::new(icons::icon(chevron_path, 12.0, toggle_color)).finish())
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
        Align::new(
            ConstrainedBox::new(Flex::row().finish())
                .with_width(52.0)
                .finish(),
        )
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
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let hover_key = format!("sidebar-menu:{label}");
    let hovered = sidebar_hover == Some(hover_key.as_str());
    let color = if disabled {
        theme::placeholder()
    } else if danger {
        theme::danger()
    } else {
        theme::text()
    };
    let action_for_click = action.clone();
    let mut row = Container::new(
        ui_text::body(label.to_string(), font)
            .with_color(color)
            .finish(),
    )
    .with_padding_left(12.0)
    .with_padding_right(12.0)
    .with_padding_top(7.0)
    .with_padding_bottom(7.0)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(7.0)));
    if hovered {
        let hover_bg = if danger {
            let mut color = theme::danger();
            color.a = 31;
            color
        } else {
            theme::accent_cool_bg(20)
        };
        row = row.with_background(hover_bg);
    }
    let row = row.finish();

    if disabled {
        return row;
    }

    let hover_key_in = hover_key.clone();
    EventHandler::new(row)
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
            ctx.dispatch_typed_action(AgentPanelAction::ClearSidebarHoverIf(hover_key.clone()));
            DispatchEventResult::PropagateToParent
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action_for_click.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn sidebar_dropdown_menu(
    font: FamilyId,
    items: Vec<(&str, bool, bool, AgentPanelAction)>,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    for (label, disabled, danger, action) in items {
        col.add_child(sidebar_menu_item(
            font,
            label,
            disabled,
            danger,
            action,
            sidebar_hover,
        ));
    }
    Container::new(
        ConstrainedBox::new(col.finish())
            .with_min_width(SIDEBAR_MENU_MIN_WIDTH)
            .with_max_width(SIDEBAR_MENU_MAX_WIDTH)
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
    sidebar_hover: Option<&str>,
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
    positioned_context_menu(x, y, sidebar_dropdown_menu(font, items, sidebar_hover))
}

/// Project-row ⋯ dropdown panel — HTML `#agent-projects-menu` (anchored in-tree).
fn project_row_menu_panel(
    font: FamilyId,
    project_id: &str,
    sidebar_hover: Option<&str>,
) -> Box<dyn Element> {
    let items = vec![(
        "删除项目",
        false,
        true,
        AgentPanelAction::OpenProjectDeleteModal(project_id.to_string()),
    )];
    EventHandler::new(sidebar_dropdown_menu(font, items, sidebar_hover))
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
    chats_flyout: Option<ChatsFlyout>,
    projects_menu_open: bool,
    chats_sort: ChatsSort,
    project_row_menu: Option<&str>,
    sidebar_hover: Option<&str>,
    project_head_hover: Option<&str>,
) -> Box<dyn Element> {
    let mut scroll_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    scroll_col.add_child(search_box(font, search, search_focused));

    let can_delete_project = projects.len() > 1;
    let projects_menu = if projects_menu_open && !active_project_id.is_empty() {
        Some(render_projects_section_menu(
            font,
            active_project_id,
            can_delete_project,
            sidebar_hover,
        ))
    } else {
        None
    };
    scroll_col.add_child(
        Container::new(sidebar_section_head(
            font,
            "项目",
            Some(("projects-more", AgentPanelAction::ToggleProjectsMenu)),
            "new-project",
            AgentPanelAction::OpenProjectCreateModal,
            "agent-edit.svg",
            projects_menu,
            sidebar_hover,
        ))
        .with_horizontal_padding(6.0)
        .with_padding_bottom(8.0)
        .finish(),
    );

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

    let chats_menu = if chats_menu_open {
        Some(render_chats_section_menu(
            font,
            chats_sort,
            chats_flyout,
            sidebar_hover,
        ))
    } else {
        None
    };
    scroll_col.add_child(
        Container::new(sidebar_section_head(
            font,
            "对话",
            Some(("chats-more", AgentPanelAction::ToggleChatsMenu)),
            "new-chat",
            AgentPanelAction::NewStandaloneChat,
            "agent-edit.svg",
            chats_menu,
            sidebar_hover,
        ))
        .with_horizontal_padding(6.0)
        .with_padding_top(12.0)
        .with_padding_bottom(8.0)
        .finish(),
    );

    let standalone = standalone_chats_sorted(sessions, archived_ids, search, chats_sort);
    if standalone.is_empty() {
        let empty = if search.trim().is_empty() {
            "暂无独立对话"
        } else {
            "无匹配独立对话"
        };
        scroll_col.add_child(
            Container::new(section_hint(empty, font))
                .with_padding_left(10.0)
                .with_padding_right(10.0)
                .with_padding_top(4.0)
                .with_padding_bottom(8.0)
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
        save_projects_state, standalone_chats, standalone_chats_sorted, upsert_archived_snapshot,
        AgentProject, AgentSession, ArchivedSessionSnapshot, ChatsSort,
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
        let dir =
            std::env::temp_dir().join(format!("wormhole-agent-projects-{}", std::process::id()));
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
        assert_eq!(
            standalone_chats_sorted(&sessions, &archived, "", ChatsSort::Created).len(),
            1
        );
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
