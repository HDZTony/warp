//! Composer「添加」面板 + chips + 演示用文件/媒体浏览器（对齐 `#agent-add-panel` /
//! `#agent-files-modal` / `#agent-media-modal`）。

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{popover_menu_header, popover_shell_with_radius};
use crate::ui::theme;
use crate::ui_text;

pub const ADD_POPOVER_WIDTH: f32 = 300.0;
pub const ADD_POPOVER_RADIUS: f32 = 14.0;
/// Align add panel to the + button (composer padding ~14).
pub const ADD_POPOVER_INSET_LEFT: f32 = 14.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerAttachment {
    pub name: String,
    pub path: String,
    pub size: String,
    pub is_folder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoFileItem {
    pub name: &'static str,
    pub path: &'static str,
    pub size: &'static str,
    pub is_folder: bool,
    pub preview: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoMediaItem {
    pub name: &'static str,
    pub size: &'static str,
}

/// HTML `fileItems` demo catalog.
pub const DEMO_FILES: &[DemoFileItem] = &[
    DemoFileItem {
        name: "Documents",
        path: "Documents/",
        size: "—",
        is_folder: true,
        preview: "文件夹 · 添加后将引用整个目录：\nDocuments/",
    },
    DemoFileItem {
        name: "Projects",
        path: "Projects/",
        size: "—",
        is_folder: true,
        preview: "文件夹 · 添加后将引用整个目录：\nProjects/",
    },
    DemoFileItem {
        name: "Shared with iPad",
        path: "Shared with iPad/",
        size: "—",
        is_folder: true,
        preview: "文件夹 · 添加后将引用整个目录：\nShared with iPad/",
    },
    DemoFileItem {
        name: "readme.txt",
        path: "readme.txt",
        size: "1.2 KB",
        is_folder: false,
        preview: "# Wormhole 共享文件夹\n\n本目录由 Iroh 保险库同步。\n终端可通过 DocTicket 加入集群并浏览此处文件。",
    },
    DemoFileItem {
        name: "iroh-ticket-format.md",
        path: "Documents/Specs/iroh-ticket-format.md",
        size: "4.8 KB",
        is_folder: false,
        preview: "---\ntitle: Iroh DocTicket 格式\n---\n\n## 票据结构\n\nDocTicket 用于集群加入与保险库只读访问。",
    },
    DemoFileItem {
        name: "release-notes.md",
        path: "release-notes.md",
        size: "2.1 KB",
        is_folder: false,
        preview: "## Wormhole 0.4.0\n\n- 终端 Tab 合并共享文件夹\n- Telegram 式聊天 UI\n- 智能体 Codex 侧栏",
    },
    DemoFileItem {
        name: "device-backup.json",
        path: "device-backup.json",
        size: "856 B",
        is_folder: false,
        preview: "{\n  \"device\": \"DESKTOP-KDSVGM5\",\n  \"cluster_id\": \"72701ff71d21\"\n}",
    },
];

/// HTML `mediaItems` demo catalog (names/sizes; thumbs are placeholders).
pub const DEMO_MEDIA: &[DemoMediaItem] = &[
    DemoMediaItem {
        name: "wormhole-hero.png",
        size: "6.3 MB",
    },
    DemoMediaItem {
        name: "codex-add-menu.png",
        size: "15 KB",
    },
    DemoMediaItem {
        name: "ui-markup.png",
        size: "407 KB",
    },
];

#[derive(Debug, Clone)]
pub struct FilesModalState {
    pub selected: usize,
}

impl FilesModalState {
    pub fn new() -> Self {
        let selected = DEMO_FILES.iter().position(|f| !f.is_folder).unwrap_or(0);
        Self { selected }
    }
}

#[derive(Debug, Clone)]
pub struct MediaModalState {
    pub selected: usize,
}

impl MediaModalState {
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

fn add_menu_item(
    font: FamilyId,
    icon_path: &'static str,
    title: &str,
    desc: Option<&str>,
    active: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let mut text_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    text_col.add_child(
        ui_text::body(title.to_string(), font)
            .with_color(theme::text())
            .finish(),
    );
    if let Some(desc) = desc {
        text_col.add_child(
            Container::new(
                ui_text::body(desc.to_string(), font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .with_padding_top(2.0)
            .finish(),
        );
    }
    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(
            Container::new(icons::icon(icon_path, 18.0, theme::text()))
                .with_padding_right(12.0)
                .with_padding_top(1.0)
                .finish(),
        )
        .with_child(Expanded::new(1.0, text_col.finish()).finish())
        .finish();
    EventHandler::new(
        Container::new(row)
            .with_uniform_padding(10.0)
            .with_background(if active {
                ColorU::new(255, 255, 255, 20)
            } else {
                ColorU::transparent_black()
            })
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.0)))
            .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

pub fn render_add_menu(font: FamilyId, goal_on: bool, plan_on: bool) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(popover_menu_header(font, "添加"));
    col.add_child(
        Container::new(add_menu_item(
            font,
            "agent-attach.svg",
            "文件和文件夹",
            None,
            false,
            AgentPanelAction::OpenFilesModal,
        ))
        .with_horizontal_padding(4.0)
        .finish(),
    );
    // HTML ships `#agent-media-modal` (demo); keep it reachable from the add menu.
    col.add_child(
        Container::new(add_menu_item(
            font,
            "agent-attach.svg",
            "图片或视频",
            None,
            false,
            AgentPanelAction::OpenMediaModal,
        ))
        .with_horizontal_padding(4.0)
        .finish(),
    );
    col.add_child(
        Container::new(add_menu_item(
            font,
            "agent-goal.svg",
            "目标",
            Some("设置智能体将持续努力实现的目标"),
            goal_on,
            AgentPanelAction::ToggleGoalMode,
        ))
        .with_horizontal_padding(4.0)
        .finish(),
    );
    col.add_child(
        Container::new(add_menu_item(
            font,
            "agent-plan.svg",
            "计划模式",
            Some("开启计划模式"),
            plan_on,
            AgentPanelAction::TogglePlanMode,
        ))
        .with_horizontal_padding(4.0)
        .with_padding_bottom(4.0)
        .finish(),
    );
    popover_shell_with_radius(ADD_POPOVER_WIDTH, ADD_POPOVER_RADIUS, col.finish())
}

fn mode_chip(
    font: FamilyId,
    icon: &'static str,
    label: &str,
    remove: AgentPanelAction,
) -> Box<dyn Element> {
    let remove_action = remove.clone();
    EventHandler::new(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(icons::icon(icon, 14.0, theme::text()))
                        .with_padding_right(6.0)
                        .finish(),
                )
                .with_child(
                    ui_text::body(label.to_string(), font)
                        .with_color(theme::text())
                        .finish(),
                )
                .with_child(
                    Container::new(
                        EventHandler::new(
                            ui_text::body(" ×".to_string(), font)
                                .with_color(theme::muted())
                                .finish(),
                        )
                        .on_left_mouse_down(move |ctx, _, _| {
                            ctx.dispatch_typed_action(remove_action.clone());
                            DispatchEventResult::StopPropagation
                        })
                        .finish(),
                    )
                    .with_padding_left(4.0)
                    .finish(),
                )
                .finish(),
        )
        .with_padding_left(8.0)
        .with_padding_right(10.0)
        .with_padding_top(6.0)
        .with_padding_bottom(6.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(remove.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn attach_chip(font: FamilyId, item: &ComposerAttachment, index: usize) -> Box<dyn Element> {
    let icon = if item.is_folder {
        "agent-folder.svg"
    } else {
        "share-file.svg"
    };
    let remove = AgentPanelAction::RemoveAttachment(index);
    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::icon(icon, 14.0, theme::muted()))
                    .with_padding_right(6.0)
                    .finish(),
            )
            .with_child(
                Shrinkable::new(
                    1.0,
                    ui_text::body(item.name.clone(), font)
                        .with_color(theme::text())
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                EventHandler::new(
                    Container::new(
                        ui_text::body(" ×".to_string(), font)
                            .with_color(theme::muted())
                            .finish(),
                    )
                    .with_padding_left(4.0)
                    .finish(),
                )
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(remove.clone());
                    DispatchEventResult::StopPropagation
                })
                .finish(),
            )
            .finish(),
    )
    .with_padding_left(8.0)
    .with_padding_right(10.0)
    .with_padding_top(6.0)
    .with_padding_bottom(6.0)
    .with_background(theme::canvas())
    .with_border(Border::all(1.0).with_border_fill(theme::border()))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
    .finish()
}

pub fn render_composer_chips(
    font: FamilyId,
    plan_mode: bool,
    goal_mode: bool,
    attachments: &[ComposerAttachment],
) -> Option<Box<dyn Element>> {
    if !plan_mode && !goal_mode && attachments.is_empty() {
        return None;
    }
    let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    if plan_mode {
        row.add_child(
            Container::new(mode_chip(
                font,
                "agent-plan.svg",
                "计划模式",
                AgentPanelAction::ClearPlanMode,
            ))
            .with_margin_right(8.0)
            .finish(),
        );
    }
    if goal_mode {
        row.add_child(
            Container::new(mode_chip(
                font,
                "agent-goal.svg",
                "目标",
                AgentPanelAction::ClearGoalMode,
            ))
            .with_margin_right(8.0)
            .finish(),
        );
    }
    for (i, item) in attachments.iter().enumerate() {
        row.add_child(
            Container::new(attach_chip(font, item, i))
                .with_margin_right(8.0)
                .finish(),
        );
    }
    Some(
        Container::new(row.finish())
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_top(10.0)
            .finish(),
    )
}

fn modal_close_btn(font: FamilyId, action: AgentPanelAction) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(
                Align::new(
                    ui_text::modal_close_glyph("×".to_string(), font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_width(28.0)
        .with_height(28.0)
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn modal_foot_btn(
    font: FamilyId,
    label: &str,
    primary: bool,
    enabled: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let color = if !enabled {
        theme::placeholder()
    } else if primary {
        theme::canvas()
    } else {
        theme::muted()
    };
    let bg = if primary && enabled {
        theme::accent_cool()
    } else {
        theme::panel_elevated()
    };
    let border = if primary && enabled {
        theme::accent_cool()
    } else {
        theme::border()
    };
    let btn = ConstrainedBox::new(
        Container::new(
            Align::new(
                ui_text::body(label.to_string(), font)
                    .with_color(color)
                    .finish(),
            )
            .finish(),
        )
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_background(bg)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish(),
    )
    .with_height(34.0)
    .finish();
    if !enabled {
        return btn;
    }
    EventHandler::new(btn)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

fn file_list_row(
    font: FamilyId,
    item: &DemoFileItem,
    index: usize,
    selected: bool,
) -> Box<dyn Element> {
    let name_color = if item.is_folder {
        theme::accent()
    } else if selected {
        theme::text()
    } else {
        theme::muted()
    };
    let icon = if item.is_folder {
        "agent-folder.svg"
    } else {
        "share-file.svg"
    };
    let row = Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(icons::icon(icon, 16.0, name_color))
                    .with_padding_right(8.0)
                    .finish(),
            )
            .with_child(
                Expanded::new(
                    1.0,
                    ui_text::body(item.name.to_string(), font)
                        .with_color(name_color)
                        .finish(),
                )
                .finish(),
            )
            .with_child(
                ui_text::body(item.size.to_string(), font)
                    .with_color(theme::muted())
                    .finish(),
            )
            .finish(),
    )
    .with_uniform_padding(10.0)
    .with_background(if selected {
        theme::accent_cool_bg(28)
    } else {
        ColorU::transparent_black()
    })
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
    .finish();
    EventHandler::new(row)
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(AgentPanelAction::SelectDemoFile(index));
            DispatchEventResult::StopPropagation
        })
        .finish()
}

pub fn render_files_modal(
    font: FamilyId,
    mono: FamilyId,
    state: &FilesModalState,
) -> Box<dyn Element> {
    let selected = DEMO_FILES.get(state.selected).unwrap_or(&DEMO_FILES[0]);
    let mut list = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    list.add_child(
        Container::new(
            ui_text::body("共享文件夹 · 本机".to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_bottom(8.0)
        .finish(),
    );
    for (i, item) in DEMO_FILES.iter().enumerate() {
        list.add_child(file_list_row(font, item, i, i == state.selected));
    }

    let mut preview = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    preview.add_child(
        Container::new(
            ui_text::mono(format!("{} · {}", selected.path, selected.size), mono)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_bottom(8.0)
        .finish(),
    );
    preview.add_child(
        Container::new(
            ui_text::mono(selected.preview.to_string(), mono)
                .with_color(theme::text())
                .finish(),
        )
        .with_uniform_padding(10.0)
        .with_background(theme::canvas())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish(),
    );

    let mut body = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    body.add_child(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::modal_dialog_title("文件和文件夹".to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(modal_close_btn(font, AgentPanelAction::CloseFilesModal))
            .finish(),
    );
    body.add_child(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    ConstrainedBox::new(list.finish())
                        .with_width(260.0)
                        .finish(),
                )
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(preview.finish())
                            .with_padding_left(12.0)
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_padding_top(12.0)
        .finish(),
    );
    body.add_child(
        Container::new(
            Flex::row()
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(modal_foot_btn(
                    font,
                    "取消",
                    false,
                    true,
                    AgentPanelAction::CloseFilesModal,
                ))
                .with_child(
                    Container::new(modal_foot_btn(
                        font,
                        "添加",
                        true,
                        true,
                        AgentPanelAction::ConfirmFilesModal,
                    ))
                    .with_margin_left(8.0)
                    .finish(),
                )
                .finish(),
        )
        .with_padding_top(16.0)
        .finish(),
    );

    let dialog = EventHandler::new(
        Container::new(
            ConstrainedBox::new(body.finish())
                .with_width(720.0)
                .with_max_height(520.0)
                .finish(),
        )
        .with_uniform_padding(20.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish();

    let scrim = EventHandler::new(
        Container::new(Flex::row().finish())
            .with_background(ColorU::new(8, 7, 11, 180))
            .finish(),
    )
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::CloseFilesModal);
        DispatchEventResult::StopPropagation
    })
    .finish();

    Stack::new()
        .with_child(scrim)
        .with_child(Align::new(dialog).finish())
        .finish()
}

fn media_thumb(
    font: FamilyId,
    item: &DemoMediaItem,
    index: usize,
    selected: bool,
) -> Box<dyn Element> {
    let border = if selected {
        theme::accent_cool()
    } else {
        theme::border()
    };
    EventHandler::new(
        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                .with_child(
                    ConstrainedBox::new(
                        Container::new(
                            Align::new(icons::icon("agent-attach.svg", 28.0, theme::muted()))
                                .finish(),
                        )
                        .with_background(theme::canvas())
                        .finish(),
                    )
                    .with_height(88.0)
                    .finish(),
                )
                .with_child(
                    Container::new(
                        ui_text::body(item.name.to_string(), font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_padding_top(6.0)
                    .finish(),
                )
                .finish(),
        )
        .with_uniform_padding(8.0)
        .with_border(Border::all(1.0).with_border_fill(border))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish(),
    )
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::SelectDemoMedia(index));
        DispatchEventResult::StopPropagation
    })
    .finish()
}

pub fn render_media_modal(font: FamilyId, state: &MediaModalState) -> Box<dyn Element> {
    let selected = DEMO_MEDIA.get(state.selected).unwrap_or(&DEMO_MEDIA[0]);
    let mut grid = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    let mut row = Flex::row();
    for (i, item) in DEMO_MEDIA.iter().enumerate() {
        row.add_child(
            Container::new(media_thumb(font, item, i, i == state.selected))
                .with_margin_right(8.0)
                .with_margin_bottom(8.0)
                .finish(),
        );
        if (i + 1) % 2 == 0 {
            grid.add_child(row.finish());
            row = Flex::row();
        }
    }
    if DEMO_MEDIA.len() % 2 != 0 {
        grid.add_child(row.finish());
    }

    let mut preview = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    preview.add_child(
        ConstrainedBox::new(
            Container::new(
                Align::new(icons::icon("agent-attach.svg", 48.0, theme::muted())).finish(),
            )
            .with_background(theme::canvas())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_height(180.0)
        .finish(),
    );
    preview.add_child(
        Container::new(
            ui_text::body(format!("{} · {}", selected.name, selected.size), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_top(10.0)
        .finish(),
    );

    let mut body = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    body.add_child(
        Flex::row()
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(
                ui_text::modal_dialog_title("图片或视频".to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .with_child(modal_close_btn(font, AgentPanelAction::CloseMediaModal))
            .finish(),
    );
    body.add_child(
        Container::new(
            Flex::row()
                .with_child(
                    ConstrainedBox::new(grid.finish())
                        .with_width(280.0)
                        .finish(),
                )
                .with_child(
                    Expanded::new(
                        1.0,
                        Container::new(preview.finish())
                            .with_padding_left(12.0)
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_padding_top(12.0)
        .finish(),
    );
    body.add_child(
        Container::new(
            Flex::row()
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(modal_foot_btn(
                    font,
                    "取消",
                    false,
                    true,
                    AgentPanelAction::CloseMediaModal,
                ))
                .with_child(
                    Container::new(modal_foot_btn(
                        font,
                        "添加",
                        true,
                        true,
                        AgentPanelAction::ConfirmMediaModal,
                    ))
                    .with_margin_left(8.0)
                    .finish(),
                )
                .finish(),
        )
        .with_padding_top(16.0)
        .finish(),
    );

    let dialog = EventHandler::new(
        Container::new(
            ConstrainedBox::new(body.finish())
                .with_width(680.0)
                .finish(),
        )
        .with_uniform_padding(20.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish();

    let scrim = EventHandler::new(
        Container::new(Flex::row().finish())
            .with_background(ColorU::new(8, 7, 11, 180))
            .finish(),
    )
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::CloseMediaModal);
        DispatchEventResult::StopPropagation
    })
    .finish();

    Stack::new()
        .with_child(scrim)
        .with_child(Align::new(dialog).finish())
        .finish()
}
