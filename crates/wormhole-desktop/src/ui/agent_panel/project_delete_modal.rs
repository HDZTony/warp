//! 删除项目确认弹窗（对齐 `desktop-current.html` `#project-delete-modal`）。

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Expanded, Flex, MainAxisAlignment, MainAxisSize,
    ParentElement, Radius, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub struct ProjectDeleteState {
    pub project_id: String,
    pub delete_local: bool,
}

impl ProjectDeleteState {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            delete_local: false,
        }
    }
}

fn cancel_button(font: FamilyId) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(
                Align::new(
                    ui_text::body("取消".to_string(), font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_height(34.0)
        .finish(),
    )
    .with_automation_label("取消")
    .with_automation_id("ai:project_delete_cancel")
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::CloseProjectDeleteModal);
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn confirm_button(font: FamilyId) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(
                Align::new(
                    ui_text::body("确认删除".to_string(), font)
                        .with_color(theme::danger())
                        .finish(),
                )
                .finish(),
            )
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_background(theme::panel_elevated())
            .with_border(Border::all(1.0).with_border_fill(theme::danger()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_height(34.0)
        .finish(),
    )
    .with_automation_label("确认删除")
    .with_automation_id("ai:project_delete_confirm")
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::ConfirmDeleteProject);
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn delete_option(
    font: FamilyId,
    title: &str,
    subtitle: &str,
    selected: bool,
    danger: bool,
    disabled: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let title_color = if disabled {
        theme::placeholder()
    } else if danger {
        theme::danger()
    } else {
        theme::text()
    };
    let border = if disabled {
        theme::border()
    } else if selected && danger {
        theme::danger()
    } else if selected {
        theme::accent_cool()
    } else {
        theme::border()
    };
    let bg = if disabled {
        theme::canvas()
    } else if selected && danger {
        ColorU::new(180, 60, 60, 28)
    } else if selected {
        theme::accent_cool_bg(20)
    } else {
        theme::canvas()
    };
    let mark = if selected { "●" } else { "○" };
    let mark_color = if disabled {
        theme::placeholder()
    } else if selected && danger {
        theme::danger()
    } else if selected {
        theme::accent_cool()
    } else {
        theme::muted()
    };

    let mut text_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    text_col.add_child(
        ui_text::body(title.to_string(), font)
            .with_color(title_color)
            .finish(),
    );
    text_col.add_child(
        Container::new(
            ui_text::body(subtitle.to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_top(2.0)
        .finish(),
    );

    let row = Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Container::new(
                    ui_text::body(mark.to_string(), font)
                        .with_color(mark_color)
                        .finish(),
                )
                .with_padding_right(10.0)
                .finish(),
            )
            .with_child(Expanded::new(1.0, text_col.finish()).finish())
            .finish(),
    )
    .with_uniform_padding(12.0)
    .with_background(bg)
    .with_border(Border::all(1.0).with_border_fill(border))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
    .finish();

    if disabled {
        return row;
    }

    EventHandler::new(row)
        .with_automation_label(title)
        .with_automation_id(if danger {
            "ai:project_delete_local"
        } else {
            "ai:project_delete_reference"
        })
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
            DispatchEventResult::StopPropagation
        })
        .finish()
}

pub fn render_project_delete_modal(
    font: FamilyId,
    mono: FamilyId,
    state: &ProjectDeleteState,
    label: &str,
    folder_path: &str,
) -> Box<dyn Element> {
    let has_path = !folder_path.trim().is_empty();
    let path_display = if has_path {
        folder_path.to_string()
    } else {
        "未绑定本地路径".to_string()
    };

    let mut body = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);

    body.add_child(
        ui_text::modal_dialog_title("删除项目".to_string(), font)
            .with_color(theme::text())
            .finish(),
    );
    body.add_child(
        Container::new(
            ui_text::body(
                "请选择如何处理本地文件夹。项目下的对话都会从侧栏移除。".to_string(),
                font,
            )
            .with_color(theme::muted())
            .finish(),
        )
        .with_padding_top(8.0)
        .with_padding_bottom(12.0)
        .finish(),
    );

    let mut target_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    target_col.add_child(
        ui_text::mono(label.to_string(), mono)
            .with_color(theme::accent_cool())
            .finish(),
    );
    target_col.add_child(
        Container::new(
            ui_text::mono(path_display, mono)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_top(4.0)
        .finish(),
    );
    body.add_child(
        Container::new(target_col.finish())
            .with_uniform_padding(10.0)
            .with_padding_bottom(4.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
    );
    body.add_child(
        Container::new(Flex::column().finish())
            .with_padding_bottom(12.0)
            .finish(),
    );

    body.add_child(delete_option(
        font,
        "仅去掉文件夹引用",
        "从侧栏移除项目与对话，本机路径中的文件全部保留。",
        !state.delete_local,
        false,
        false,
        AgentPanelAction::SetProjectDeleteMode(false),
    ));
    body.add_child(
        Container::new(delete_option(
            font,
            "同时删除本地文件夹",
            "移除引用，并删除本机路径中的文件。此操作不可撤销。",
            state.delete_local,
            true,
            !has_path,
            AgentPanelAction::SetProjectDeleteMode(true),
        ))
        .with_padding_top(8.0)
        .finish(),
    );

    body.add_child(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(cancel_button(font))
                .with_child(
                    Container::new(confirm_button(font))
                        .with_margin_left(8.0)
                        .finish(),
                )
                .finish(),
        )
        .with_padding_top(18.0)
        .finish(),
    );

    let dialog = EventHandler::new(
        Container::new(
            ConstrainedBox::new(body.finish())
                .with_width(420.0)
                .finish(),
        )
        .with_uniform_padding(24.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish(),
    )
    .with_automation_label("删除项目对话框")
    .with_automation_id("ai:project_delete_dialog")
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish();

    let scrim = EventHandler::new(
        Container::new(Flex::row().finish())
            .with_background(ColorU::new(8, 7, 11, 180))
            .finish(),
    )
    .with_automation_label("关闭删除项目")
    .with_automation_id("ai:project_delete_scrim")
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::CloseProjectDeleteModal);
        DispatchEventResult::StopPropagation
    })
    .finish();

    Stack::new()
        .with_child(scrim)
        .with_child(Align::new(dialog).finish())
        .finish()
}
