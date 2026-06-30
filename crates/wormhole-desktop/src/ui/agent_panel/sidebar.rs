//! Agent panel sidebar — Projects + Chats (`desktop-current.html`).

use std::collections::HashSet;
use std::path::Path;

use pathfinder_color::ColorU;
use warpui::elements::Fill;
use warpui::elements::{
    Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Expanded, Flex, MainAxisSize,
    ParentElement, Radius, ScrollbarWidth, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{
    agent_row_active_bg, agent_sidebar_bg, agent_sidebar_label, section_hint, AGENT_ICON_BTN_RADIUS,
    AGENT_ROW_RADIUS, SECTION_PADDING,
};
use crate::ui::theme;
use crate::ui_text;

pub const SIDEBAR_WIDTH: f32 = 260.0;
const ARCHIVE_FILE: &str = "agent-archived.json";

#[derive(Debug, Clone)]
pub struct AgentProject {
    pub id: String,
    pub label: String,
    pub time: String,
}

#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: String,
    pub label: String,
    pub project_id: String,
    pub time: String,
    pub running: bool,
    pub prompt: String,
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

fn session_matches_query(session: &AgentSession, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hay = format!("{} {}", session.label, session.prompt).to_lowercase();
    hay.contains(&q)
}

fn icon_button(action: AgentPanelAction, icon_path: &'static str) -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(
            EventHandler::new(icons::agent_icon(icon_path, theme::muted()))
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_width(26.0)
        .with_height(26.0)
        .finish(),
    )
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ICON_BTN_RADIUS)))
    .finish()
}

fn section_head(font: FamilyId, label: &str, new_action: AgentPanelAction) -> Box<dyn Element> {
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Shrinkable::new(1.0, agent_sidebar_label(label.to_string(), font)).finish(),
        )
        .with_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(icon_button(AgentPanelAction::SidebarMore, "agent-more.svg"))
                .with_child(icon_button(new_action, "agent-new.svg"))
                .finish(),
        )
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

fn row_item(
    font: FamilyId,
    label: String,
    time: String,
    active: bool,
    show_spinner: bool,
    archived: bool,
    action: AgentPanelAction,
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
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max);
    if show_spinner {
        row.add_child(chat_spinner());
    }
    row.add_child(
        Shrinkable::new(
            1.0,
            ui_text::body(label, font)
                .with_color(fg)
                .finish(),
        )
        .finish(),
    );
    row.add_child(
        Container::new(
            ui_text::mono(time, font)
                .with_color(time_color)
                .finish(),
        )
        .with_horizontal_margin(6.0)
        .finish(),
    );
    Container::new(
        EventHandler::new(row.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_padding_left(10.0)
    .with_padding_right(10.0)
    .with_padding_top(7.0)
    .with_padding_bottom(7.0)
    .with_background(bg)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
    .finish()
}

fn archive_toggle(font: FamilyId, count: usize, expanded: bool) -> Box<dyn Element> {
    let chevron = if expanded { "▼" } else { "▶" };
    let label = format!("{chevron} 已归档");
    Container::new(
        EventHandler::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    ui_text::body(label, font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_child(
                    Container::new(
                        ui_text::mono(count.to_string(), font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_uniform_padding(4.0)
                    .with_background(theme::accent_cool_bg(24))
                    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.0)))
                    .with_horizontal_margin(8.0)
                    .finish(),
                )
                .finish(),
        )
        .on_left_mouse_down(|ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::ToggleArchiveSection);
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_padding_left(10.0)
    .with_padding_right(10.0)
    .with_padding_top(8.0)
    .with_padding_bottom(4.0)
    .finish()
}

pub fn render_sidebar(
    font: FamilyId,
    scroll: ClippedScrollStateHandle,
    projects: &[AgentProject],
    sessions: &[AgentSession],
    active_project_id: &str,
    active_session_id: &str,
    search: &str,
    search_focused: bool,
    archived_ids: &HashSet<String>,
    archive_expanded: bool,
) -> Box<dyn Element> {
    let mut scroll_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    scroll_col.add_child(
        Container::new(section_head(font, "Projects", AgentPanelAction::NewProject))
            .with_horizontal_padding(6.0)
            .with_padding_bottom(8.0)
            .finish(),
    );

    if projects.is_empty() {
        scroll_col.add_child(
            Container::new(section_hint("暂无项目，点击 + 新建", font))
                .with_horizontal_padding(10.0)
                .with_padding_bottom(4.0)
                .finish(),
        );
    } else {
        for project in projects {
            let active = project.id == active_project_id;
            scroll_col.add_child(
                Container::new(row_item(
                    font,
                    project.label.clone(),
                    project.time.clone(),
                    active,
                    false,
                    false,
                    AgentPanelAction::SelectProject(project.id.clone()),
                ))
                .with_horizontal_padding(2.0)
                .finish(),
            );
        }
    }

    scroll_col.add_child(
        Container::new(section_head(font, "Chats", AgentPanelAction::NewThread))
            .with_horizontal_padding(6.0)
            .with_padding_top(12.0)
            .with_padding_bottom(8.0)
            .finish(),
    );

    let search_border = if search_focused {
        theme::accent_cool()
    } else {
        theme::border()
    };
    let search_text = if search.is_empty() {
        "Search chats…".to_string()
    } else {
        search.to_string()
    };
    let search_color = if search.is_empty() && !search_focused {
        theme::placeholder()
    } else {
        theme::text()
    };
    scroll_col.add_child(
        Container::new(
            Flex::row()
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
                .finish(),
        )
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(7.0)
        .with_padding_bottom(7.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_color(search_border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
        .with_horizontal_margin(2.0)
        .with_margin_bottom(8.0)
        .finish(),
    );

    let project_selected = !active_project_id.is_empty();
    let active_sessions: Vec<_> = if project_selected {
        sessions
            .iter()
            .filter(|s| {
                s.project_id == active_project_id
                    && !archived_ids.contains(&s.id)
                    && session_matches_query(s, search)
            })
            .collect()
    } else {
        Vec::new()
    };

    if !project_selected {
        scroll_col.add_child(
            Container::new(section_hint("请先新建或选择项目", font))
                .with_horizontal_padding(10.0)
                .finish(),
        );
    } else if active_sessions.is_empty() {
        let empty = if search.trim().is_empty() {
            "此项目暂无会话"
        } else {
            "没有匹配的会话"
        };
        scroll_col.add_child(
            Container::new(section_hint(empty, font))
                .with_horizontal_padding(10.0)
                .finish(),
        );
    } else {
        for session in active_sessions {
            let active = session.id == active_session_id;
            let show_spinner = session.running && active;
            scroll_col.add_child(
                Container::new(row_item(
                    font,
                    session.label.clone(),
                    session.time.clone(),
                    active,
                    show_spinner,
                    false,
                    AgentPanelAction::SelectSession(session.id.clone()),
                ))
                .with_horizontal_padding(2.0)
                .finish(),
            );
        }
    }

    let archived_sessions: Vec<_> = if project_selected {
        sessions
            .iter()
            .filter(|s| s.project_id == active_project_id && archived_ids.contains(&s.id))
            .collect()
    } else {
        Vec::new()
    };

    scroll_col.add_child(archive_toggle(font, archived_sessions.len(), archive_expanded));
    if archive_expanded {
        if archived_sessions.is_empty() {
            scroll_col.add_child(
                Container::new(section_hint("暂无归档会话", font))
                    .with_horizontal_padding(10.0)
                    .with_padding_top(4.0)
                    .finish(),
            );
        } else {
            for session in archived_sessions {
                let active = session.id == active_session_id;
                scroll_col.add_child(
                    Container::new(row_item(
                        font,
                        session.label.clone(),
                        session.time.clone(),
                        active,
                        false,
                        true,
                        AgentPanelAction::SelectSession(session.id.clone()),
                    ))
                    .with_horizontal_padding(2.0)
                    .with_padding_top(2.0)
                    .finish(),
                );
            }
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
    use super::{load_archived_ids, save_archived_ids, AgentSession};

    #[test]
    fn archived_ids_roundtrip() {
        let dir = std::env::temp_dir().join(format!("wormhole-agent-archive-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut ids = std::collections::HashSet::new();
        ids.insert("session-a".into());
        save_archived_ids(&dir, &ids);
        let loaded = load_archived_ids(&dir);
        assert!(loaded.contains("session-a"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn agent_session_has_no_body_field() {
        let session = AgentSession {
            id: "id".into(),
            label: "label".into(),
            project_id: "p".into(),
            time: "now".into(),
            running: false,
            prompt: String::new(),
        };
        assert_eq!(session.label, "label");
    }
}
