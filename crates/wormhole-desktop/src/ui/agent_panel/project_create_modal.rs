//! 创建项目两步弹窗（对齐 `desktop-current.html` `#project-create-modal`）。

use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, EventHandler, Flex, MainAxisAlignment, MainAxisSize, ParentElement,
    Radius, Shrinkable, Stack,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectCreateStep {
    TypeSelect,
    Name,
}

#[derive(Debug, Clone)]
pub struct ProjectCreateState {
    pub step: ProjectCreateStep,
    pub name: String,
    pub name_focused: bool,
    pub name_invalid: bool,
}

impl ProjectCreateState {
    pub fn new_type_select() -> Self {
        Self {
            step: ProjectCreateStep::TypeSelect,
            name: "New project".into(),
            name_focused: false,
            name_invalid: false,
        }
    }
}

fn submit_button(font: FamilyId, label: &str, action: AgentPanelAction) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(
                Align::new(
                    ui_text::body(label.to_string(), font)
                        .with_color(theme::canvas())
                        .finish(),
                )
                .finish(),
            )
            .with_padding_left(16.0)
            .with_padding_right(16.0)
            .with_background(theme::accent_cool())
            .with_border(Border::all(1.0).with_border_fill(theme::border()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .with_height(34.0)
        .finish(),
    )
    .with_automation_label(label)
    .with_automation_id(format!("ai:project_create:{label}"))
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn link_button(font: FamilyId, label: &str, action: AgentPanelAction) -> Box<dyn Element> {
    EventHandler::new(
        ui_text::body(label.to_string(), font)
            .with_color(theme::accent_cool())
            .finish(),
    )
    .with_automation_label(label)
    .with_automation_id(format!("ai:project_create:{label}"))
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn field_label(font: FamilyId, label: &str) -> Box<dyn Element> {
    Container::new(
        ui_text::body(label.to_string(), font)
            .with_color(theme::muted())
            .finish(),
    )
    .with_padding_bottom(6.0)
    .finish()
}

fn text_field(
    font: FamilyId,
    value: &str,
    placeholder: &str,
    invalid: bool,
    focused: bool,
    focus_action: AgentPanelAction,
) -> Box<dyn Element> {
    let border = if invalid {
        theme::danger()
    } else if focused {
        theme::accent_cool()
    } else {
        theme::border()
    };
    let display = if value.is_empty() {
        placeholder.to_string()
    } else {
        value.to_string()
    };
    let color = if value.is_empty() && !focused {
        theme::placeholder()
    } else {
        theme::text()
    };
    EventHandler::new(
        Container::new(ui_text::body(display, font).with_color(color).finish())
            .with_padding_left(12.0)
            .with_padding_right(12.0)
            .with_padding_top(10.0)
            .with_padding_bottom(10.0)
            .with_background(theme::canvas())
            .with_border(Border::all(1.0).with_border_color(border))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
    )
    .with_automation_label("项目名称")
    .with_automation_id("ai:project_create_name")
    .on_left_mouse_down(move |ctx, _, _| {
        ctx.dispatch_typed_action(focus_action.clone());
        DispatchEventResult::StopPropagation
    })
    .finish()
}

fn dialog_head(font: FamilyId, title: &str) -> Box<dyn Element> {
    const CLOSE_BTN: f32 = 28.0;
    Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
        .with_main_axis_size(MainAxisSize::Max)
        .with_child(
            Shrinkable::new(
                1.0,
                ui_text::modal_dialog_title(title.to_string(), font)
                    .with_color(theme::text())
                    .finish(),
            )
            .finish(),
        )
        .with_child(
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
                .with_width(CLOSE_BTN)
                .with_height(CLOSE_BTN)
                .finish(),
            )
            .with_automation_label("关闭")
            .with_automation_id("ai:project_create_close")
            .on_left_mouse_down(|ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::CloseProjectCreateModal);
                DispatchEventResult::StopPropagation
            })
            .finish(),
        )
        .finish()
}

fn type_select_step(font: FamilyId) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);
    col.add_child(dialog_head(font, "创建项目"));
    col.add_child(
        Container::new(
            ui_text::body("选择一个项目类型开始。".to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_top(8.0)
        .with_padding_bottom(12.0)
        .finish(),
    );
    col.add_child(field_label(font, "项目类型"));
    col.add_child(
        Container::new(
            ui_text::body("本地文件夹项目".to_string(), font)
                .with_color(theme::text())
                .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_top(10.0)
        .with_padding_bottom(10.0)
        .with_background(theme::accent_cool_bg(24))
        .with_border(Border::all(1.0).with_border_fill(theme::accent_cool()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish(),
    );
    col.add_child(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(link_button(
                    font,
                    "使用现有文件夹",
                    AgentPanelAction::ProjectCreateUseExistingFolder,
                ))
                .with_child(submit_button(
                    font,
                    "下一步",
                    AgentPanelAction::ProjectCreateNext,
                ))
                .finish(),
        )
        .with_padding_top(16.0)
        .finish(),
    );
    col.finish()
}

fn name_step(font: FamilyId, state: &ProjectCreateState) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);
    col.add_child(dialog_head(font, "为项目命名"));
    col.add_child(
        Container::new(
            ui_text::body("保持简短且易识别".to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_top(8.0)
        .with_padding_bottom(12.0)
        .finish(),
    );
    col.add_child(field_label(font, "项目名称"));
    col.add_child(text_field(
        font,
        &state.name,
        "New project",
        state.name_invalid,
        state.name_focused,
        AgentPanelAction::FocusProjectCreateName,
    ));
    col.add_child(
        Container::new(
            Align::new(submit_button(
                font,
                "创建",
                AgentPanelAction::ProjectCreateSubmit,
            ))
            .right()
            .finish(),
        )
        .with_padding_top(16.0)
        .finish(),
    );
    col.finish()
}

pub fn render_project_create_modal(font: FamilyId, state: &ProjectCreateState) -> Box<dyn Element> {
    let body = match state.step {
        ProjectCreateStep::TypeSelect => type_select_step(font),
        ProjectCreateStep::Name => name_step(font, state),
    };
    let dialog = Container::new(ConstrainedBox::new(body).with_max_width(480.0).finish())
        .with_uniform_padding(20.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(12.0)))
        .finish();

    let scrim = EventHandler::new(
        Container::new(Flex::row().finish())
            .with_background(ColorU::new(8, 7, 11, 150))
            .finish(),
    )
    .with_automation_label("关闭创建项目")
    .with_automation_id("ai:project_create_scrim")
    .on_left_mouse_down(|ctx, _, _| {
        ctx.dispatch_typed_action(AgentPanelAction::CloseProjectCreateModal);
        DispatchEventResult::StopPropagation
    })
    .finish();

    Stack::new()
        .with_child(scrim)
        .with_child(Align::new(dialog).finish())
        .finish()
}
