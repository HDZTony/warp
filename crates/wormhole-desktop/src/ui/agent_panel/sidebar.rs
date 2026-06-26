//! Agent panel sidebar — Projects + Chats (`desktop-current.html`).

use pathfinder_color::ColorU;
use warpui::elements::{
    Border, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex,
    MainAxisSize, ParentElement, Radius, Shrinkable,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::panel_primitives::{section_hint, HUD_RADIUS};
use crate::ui::theme;
use crate::ui_text;

pub const SIDEBAR_WIDTH: f32 = 220.0;

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

pub fn seed_projects() -> Vec<AgentProject> {
    vec![
        AgentProject {
            id: "wormhole".into(),
            label: "Wormhole 集群".into(),
            time: "8m".into(),
        },
        AgentProject {
            id: "codex-compare".into(),
            label: "比较 Codex Project 和 Chat".into(),
            time: "2d".into(),
        },
    ]
}

pub fn seed_sessions() -> Vec<AgentSession> {
    vec![
        AgentSession {
            id: "sync-share".into(),
            label: "同步终端共享文件夹".into(),
            project_id: "wormhole".into(),
            time: "8m".into(),
            running: true,
            prompt: "扫描三台终端的共享文件夹，把未同步的 Specs 文档全部拉取到本机。".into(),
        },
        AgentSession {
            id: "ipad-display".into(),
            label: "配置 iPad 虚拟显示器".into(),
            project_id: "wormhole".into(),
            time: "32m".into(),
            running: false,
            prompt: "检查 iPad 接收端分辨率与 USB 配对状态，并给出推流参数建议。".into(),
        },
        AgentSession {
            id: "cluster-health".into(),
            label: "集群节点健康检查".into(),
            project_id: "wormhole".into(),
            time: "1h".into(),
            running: false,
            prompt: "检查三台终端的在线状态、延迟与最后同步时间。".into(),
        },
        AgentSession {
            id: "vault-cache".into(),
            label: "清理保险库缓存".into(),
            project_id: "wormhole".into(),
            time: "1d".into(),
            running: false,
            prompt: "列出本地保险库中超过 30 天未访问的缓存文件，并生成可删除清单。".into(),
        },
        AgentSession {
            id: "docticket".into(),
            label: "DocTicket 导出说明".into(),
            project_id: "wormhole".into(),
            time: "1d".into(),
            running: false,
            prompt: "生成一份 DocTicket 分享给同事的步骤说明，包含权限与过期时间。".into(),
        },
        AgentSession {
            id: "file-expenses".into(),
            label: "File expenses".into(),
            project_id: "codex-compare".into(),
            time: "14m".into(),
            running: false,
            prompt: "整理项目目录下的 expense 相关文件，并生成摘要。".into(),
        },
        AgentSession {
            id: "hevc-tool".into(),
            label: "HEVC 转码任务".into(),
            project_id: "wormhole".into(),
            time: "5d".into(),
            running: false,
            prompt: "把 Projects/demo.hevc 转成可在 iPad 预览的片段。".into(),
        },
    ]
}

fn session_matches_query(session: &AgentSession, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hay = format!("{} {}", session.label, session.prompt).to_lowercase();
    hay.contains(&q)
}

fn section_head(font: FamilyId, label: &str, new_action: AgentPanelAction) -> Box<dyn Element> {
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Shrinkable::new(
                1.0,
                ui_text::hud_title(label.to_string(), font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish(),
        )
        .with_child(
            Container::new(
                EventHandler::new(
                    ui_text::body("+", font)
                        .with_color(theme::accent_cool())
                        .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(new_action.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .with_uniform_padding(4.0)
            .finish(),
        )
        .finish()
}

fn row_item(
    font: FamilyId,
    label: String,
    time: String,
    active: bool,
    running: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let (fg, bg) = if active {
        (theme::accent_cool(), theme::accent_cool_bg_default())
    } else {
        (theme::text(), ColorU::transparent_black())
    };
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Max);
    if running && active {
        row.add_child(
            Container::new(
                ui_text::body("◌", font)
                    .with_color(theme::accent())
                    .finish(),
            )
            .with_horizontal_margin(4.0)
            .finish(),
        );
    }
    row.add_child(
        Shrinkable::new(
            1.0,
            ui_text::body(label, font).with_color(fg).finish(),
        )
        .finish(),
    );
    row.add_child(
        ui_text::body(time, font)
            .with_color(theme::muted())
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
    .with_uniform_padding(8.0)
    .with_background(bg)
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(HUD_RADIUS)))
    .finish()
}

pub fn render_sidebar(
    font: FamilyId,
    projects: &[AgentProject],
    sessions: &[AgentSession],
    active_project_id: &str,
    active_session_id: &str,
    search: &str,
    search_focused: bool,
) -> Box<dyn Element> {
    let mut scroll_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    scroll_col.add_child(
        Container::new(section_head(
            font,
            "Projects",
            AgentPanelAction::NewProject,
        ))
        .with_horizontal_padding(8.0)
        .with_vertical_padding(6.0)
        .finish(),
    );

    for project in projects {
        let active = project.id == active_project_id;
        scroll_col.add_child(
            Container::new(row_item(
                font,
                project.label.clone(),
                project.time.clone(),
                active,
                false,
                AgentPanelAction::SelectProject(project.id.clone()),
            ))
            .with_horizontal_padding(6.0)
            .finish(),
        );
    }

    scroll_col.add_child(
        Container::new(section_head(font, "Chats", AgentPanelAction::NewThread))
            .with_horizontal_padding(8.0)
            .with_vertical_padding(10.0)
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
        .with_uniform_padding(8.0)
        .with_background(theme::bg())
        .with_border(Border::all(1.0).with_border_color(search_border))
        .with_horizontal_margin(8.0)
        .with_vertical_margin(4.0)
        .finish(),
    );

    let filtered: Vec<_> = sessions
        .iter()
        .filter(|s| s.project_id == active_project_id && session_matches_query(s, search))
        .collect();

    if filtered.is_empty() {
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
        for session in filtered {
            let active = session.id == active_session_id;
            scroll_col.add_child(
                Container::new(row_item(
                    font,
                    session.label.clone(),
                    session.time.clone(),
                    active,
                    session.running,
                    AgentPanelAction::SelectSession(session.id.clone()),
                ))
                .with_horizontal_padding(6.0)
                .finish(),
            );
        }
    }

    Container::new(scroll_col.finish())
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
}
