//! Agent panel sidebar — Projects + Chats (`desktop-current.html`).

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
    pub model: String,
    pub body: String,
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
            model: "GPT-5.5".into(),
            prompt: "扫描三台终端的共享文件夹，把未同步的 Specs 文档全部拉取到本机。".into(),
            body: "正在检查集群内三台终端的共享目录，并比对本地副本状态：\n\n\
                   • Windows 11 — Documents/Specs 下 2 个文件待同步\n\
                   • iOS 18 — Wormhole/ 已是最新，跳过\n\
                   • iPadOS 18 — Shared from PC/ 发现 1 个新文件\n\n\
                   接下来会依次写入本地保险库，并在终端 Tab 更新同步状态。"
                .into(),
        },
        AgentSession {
            id: "ipad-display".into(),
            label: "配置 iPad 虚拟显示器".into(),
            project_id: "wormhole".into(),
            time: "32m".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "检查 iPad 接收端分辨率与 USB 配对状态，并给出推流参数建议。".into(),
            body: "已读取 iPadOS 18 终端连接信息：\n\n\
                   • USB 未直连，当前通过集群中继\n\
                   • 推荐推流 2560×1600 · HEVC · 60fps\n\
                   • 触控工具栏与全屏模式已就绪\n\n\
                   可在终端 Tab 双击 iPad 查看 Display Cache 目录。"
                .into(),
        },
        AgentSession {
            id: "cluster-health".into(),
            label: "集群节点健康检查".into(),
            project_id: "wormhole".into(),
            time: "1h".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "检查三台终端的在线状态、延迟与最后同步时间。".into(),
            body: "集群成员 3 / 在线 3：\n\n\
                   • 01KT339ZT7NH24ZVCCF5TRP68K — 本机 · 延迟 12ms\n\
                   • 01KX8HM2P4NQ7W9R3F6YJ5T1D — USB 已配对 · 延迟 18ms\n\
                   • 01KQ2M8V4N6P3R9S7T1W5Y0Z — 在线 · 延迟 24ms\n\n\
                   无节点离线，Iroh 同步通道正常。"
                .into(),
        },
        AgentSession {
            id: "vault-cache".into(),
            label: "清理保险库缓存".into(),
            project_id: "wormhole".into(),
            time: "1d".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "列出本地保险库中超过 30 天未访问的缓存文件，并生成可删除清单。".into(),
            body: "扫描完成。发现 12 个缓存条目，合计约 840 MB：\n\n\
                   • Display Cache — 6 项\n\
                   • HEVC 转码临时文件 — 4 项\n\
                   • DocTicket 预览缓存 — 2 项\n\n\
                   删除前会保留共享文件夹中的活跃副本。"
                .into(),
        },
        AgentSession {
            id: "docticket".into(),
            label: "DocTicket 导出说明".into(),
            project_id: "wormhole".into(),
            time: "1d".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "生成一份 DocTicket 分享给同事的步骤说明，包含权限与过期时间。".into(),
            body: "DocTicket 通过 Iroh 保险库生成只读票据：\n\n\
                   1. 在共享文件夹选中文件 → 复制 DocTicket\n\
                   2. 票据默认 7 天有效，可限制只读\n\
                   3. 对方粘贴票据即可拉取，无需账户\n\n\
                   已写入 readme 草稿，可导出为 Markdown。"
                .into(),
        },
        AgentSession {
            id: "file-expenses".into(),
            label: "File expenses".into(),
            project_id: "codex-compare".into(),
            time: "14m".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "整理项目目录下的 expense 相关文件，并生成摘要。".into(),
            body: "已在比较 Codex Project 和 Chat 项目中扫描 expense 文件：\n\n\
                   • receipts/ — 3 张图片\n\
                   • expenses.csv — 12 行记录\n\n\
                   可导出为 Markdown 摘要或复制到剪贴板。"
                .into(),
        },
        AgentSession {
            id: "hevc-tool".into(),
            label: "HEVC 转码任务".into(),
            project_id: "wormhole".into(),
            time: "5d".into(),
            running: false,
            model: "GPT-5.5".into(),
            prompt: "把 Projects/demo.hevc 转成可在 iPad 预览的片段。".into(),
            body: "转码任务已排队。输入 1920×1080 · 8 Mbps，预计 2 分钟完成。\n\n\
                   输出路径：Projects/exports/demo-preview.mp4"
                .into(),
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
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let fg = if active {
        theme::text()
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

pub fn render_sidebar(
    font: FamilyId,
    scroll: ClippedScrollStateHandle,
    projects: &[AgentProject],
    sessions: &[AgentSession],
    active_project_id: &str,
    active_session_id: &str,
    search: &str,
    search_focused: bool,
) -> Box<dyn Element> {
    let mut scroll_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

    scroll_col.add_child(
        Container::new(section_head(font, "Projects", AgentPanelAction::NewProject))
            .with_horizontal_padding(6.0)
            .with_padding_bottom(8.0)
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
            .with_horizontal_padding(2.0)
            .finish(),
        );
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
            let show_spinner = session.running && active;
            scroll_col.add_child(
                Container::new(row_item(
                    font,
                    session.label.clone(),
                    session.time.clone(),
                    active,
                    show_spinner,
                    AgentPanelAction::SelectSession(session.id.clone()),
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
