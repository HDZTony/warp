//! Composer access / model popovers — floating HUD menus above the composer bar.

use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::panel_primitives::{agent_sidebar_label, section_hint, AGENT_ROW_RADIUS};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::warp_embed_prefs::{AgentAccessMode, PreferredAgent};

pub const ACCESS_POPOVER_WIDTH: f32 = 300.0;
pub const MODEL_POPOVER_WIDTH: f32 = 220.0;

pub fn access_label(mode: AgentAccessMode) -> &'static str {
    match mode {
        AgentAccessMode::FullAccess => "完全访问",
        AgentAccessMode::WorkspaceWrite => "工作区写入",
    }
}

pub fn model_label(agent: PreferredAgent) -> &'static str {
    match agent {
        PreferredAgent::Codex => "GPT-5.5",
        PreferredAgent::Cursor => "Cursor",
    }
}

fn popover_shell(width: f32, body: Box<dyn Element>) -> Box<dyn Element> {
    EventHandler::new(
        ConstrainedBox::new(
            Container::new(body)
                .with_background(theme::panel_elevated())
                .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AGENT_ROW_RADIUS)))
                .finish(),
        )
        .with_min_width(width)
        .with_width(width)
        .finish(),
    )
    .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
    .finish()
}

fn menu_header(font: FamilyId, title: impl Into<String>) -> Box<dyn Element> {
    Container::new(agent_sidebar_label(title.into(), font))
        .with_padding_left(14.0)
        .with_padding_right(14.0)
        .with_padding_top(12.0)
        .with_padding_bottom(8.0)
        .finish()
}

fn menu_option(
    font: FamilyId,
    label: impl Into<String>,
    selected: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let label = label.into();
    let label_el = ui_text::body(label, font)
        .with_color(if selected {
            theme::text()
        } else {
            theme::muted()
        })
        .finish();
    let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    row.add_child(label_el);
    if selected {
        row.add_child(
            Container::new(
                ui_text::body("✓", font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_padding_left(8.0)
            .finish(),
        );
    }
    Container::new(
        EventHandler::new(row.finish())
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_padding_left(14.0)
    .with_padding_right(14.0)
    .with_padding_top(10.0)
    .with_padding_bottom(10.0)
    .with_background(if selected {
        theme::accent_cool_bg(32)
    } else {
        pathfinder_color::ColorU::transparent_black()
    })
    .finish()
}

fn access_option(
    font: FamilyId,
    title: impl Into<String>,
    detail: impl Into<String>,
    selected: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let title = title.into();
    let detail = detail.into();
    let mut title_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    title_row.add_child(
        ui_text::body(title, font)
            .with_color(if selected {
                theme::text()
            } else {
                theme::muted()
            })
            .finish(),
    );
    if selected {
        title_row.add_child(
            Container::new(
                ui_text::body("✓", font)
                    .with_color(theme::accent_cool())
                    .finish(),
            )
            .with_padding_left(8.0)
            .finish(),
        );
    }
    let body = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(title_row.finish())
        .with_child(
            Container::new(section_hint(detail, font))
                .with_padding_top(4.0)
                .finish(),
        )
        .finish();
    Container::new(
        EventHandler::new(body)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(action.clone());
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_padding_left(14.0)
    .with_padding_right(14.0)
    .with_padding_top(10.0)
    .with_padding_bottom(10.0)
    .with_background(if selected {
        theme::accent_cool_bg(32)
    } else {
        pathfinder_color::ColorU::transparent_black()
    })
    .finish()
}

pub fn render_access_menu(font: FamilyId, mode: AgentAccessMode) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(menu_header(font, "访问权限"));
    col.add_child(access_option(
        font,
        access_label(AgentAccessMode::FullAccess),
        "可不受限制地访问互联网和您电脑上的任何文件",
        mode == AgentAccessMode::FullAccess,
        AgentPanelAction::SelectAccessMode(AgentAccessMode::FullAccess),
    ));
    col.add_child(access_option(
        font,
        access_label(AgentAccessMode::WorkspaceWrite),
        "仅在工作区内写入与修改文件",
        mode == AgentAccessMode::WorkspaceWrite,
        AgentPanelAction::SelectAccessMode(AgentAccessMode::WorkspaceWrite),
    ));
    popover_shell(ACCESS_POPOVER_WIDTH, col.finish())
}

pub fn render_model_menu(font: FamilyId, agent: PreferredAgent) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    col.add_child(menu_header(font, "模型"));
    col.add_child(menu_option(
        font,
        model_label(PreferredAgent::Codex),
        agent == PreferredAgent::Codex,
        AgentPanelAction::SelectAgent(PreferredAgent::Codex),
    ));
    col.add_child(menu_option(
        font,
        model_label(PreferredAgent::Cursor),
        agent == PreferredAgent::Cursor,
        AgentPanelAction::SelectAgent(PreferredAgent::Cursor),
    ));
    popover_shell(MODEL_POPOVER_WIDTH, col.finish())
}
