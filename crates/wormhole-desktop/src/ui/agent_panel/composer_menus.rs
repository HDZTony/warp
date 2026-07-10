//! Composer access / model-rate popovers — floating HUD menus above the composer bar.

use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use super::AgentPanelAction;
use crate::ui::panel_primitives::{popover_menu_header, popover_shell, section_hint};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::warp_embed_prefs::{AgentAccessMode, AgentModelRate};

pub const ACCESS_POPOVER_WIDTH: f32 = 300.0;
pub const MODEL_RATE_POPOVER_WIDTH: f32 = 280.0;

pub fn access_label(mode: AgentAccessMode) -> &'static str {
    match mode {
        AgentAccessMode::FullAccess => "完全访问",
        AgentAccessMode::WorkspaceWrite => "工作区写入",
    }
}

pub fn composer_model_chip_label(rate: AgentModelRate) -> &'static str {
    rate.chip_label()
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
    col.add_child(popover_menu_header(font, "访问权限"));
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

fn model_radio_dot() -> Box<dyn Element> {
    use warpui::elements::Align;
    Align::new(
        Container::new(
            ConstrainedBox::new(Flex::row().finish())
                .with_width(8.0)
                .with_height(8.0)
                .finish(),
        )
        .with_background(theme::accent_cool())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.0)))
        .finish(),
    )
    .finish()
}

fn model_radio(selected: bool) -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(if selected {
            model_radio_dot()
        } else {
            Flex::row().finish()
        })
        .with_width(16.0)
        .with_height(16.0)
        .finish(),
    )
    .with_border(Border::all(1.5).with_border_fill(if selected {
        theme::accent_cool()
    } else {
        theme::border_bright()
    }))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
    .finish()
}

fn model_rate_option(font: FamilyId, rate: AgentModelRate, selected: bool) -> Box<dyn Element> {
    let speed_color = if selected {
        theme::accent()
    } else {
        theme::muted()
    };
    // Same shape as access_option: single Min column, no Stretch/Max/Expanded.
    // Nested row + Stretch column under Align popover measure → infinite width panic.
    let mut title_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    title_row.add_child(model_radio(selected));
    title_row.add_child(
        Container::new(
            ui_text::body("GPT-5.5", font)
                .with_color(theme::text())
                .finish(),
        )
        .with_padding_left(10.0)
        .finish(),
    );
    title_row.add_child(
        Container::new(
            ui_text::body(rate.rate_label(), font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_left(6.0)
        .finish(),
    );
    title_row.add_child(
        Container::new(
            ui_text::body(rate.speed_label(), font)
                .with_color(speed_color)
                .finish(),
        )
        .with_padding_left(12.0)
        .finish(),
    );
    let body = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(title_row.finish())
        .with_child(
            Container::new(section_hint(rate.option_desc(), font))
                .with_padding_top(4.0)
                .finish(),
        )
        .finish();

    Container::new(
        EventHandler::new(body)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SelectModelRate(rate));
                DispatchEventResult::StopPropagation
            })
            .finish(),
    )
    .with_padding_left(14.0)
    .with_padding_right(14.0)
    .with_padding_top(10.0)
    .with_padding_bottom(10.0)
    .with_background(if selected {
        theme::accent_cool_bg(26)
    } else {
        pathfinder_color::ColorU::transparent_black()
    })
    .finish()
}

fn model_rate_footnote(font: FamilyId, rate: AgentModelRate) -> Box<dyn Element> {
    let prefix = format!("{} {}", rate.rate_label(), rate.speed_label());
    let mut line = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Start);
    line.add_child(
        ui_text::body(prefix, font)
            .with_color(theme::accent_cool())
            .finish(),
    );
    line.add_child(
        Container::new(
            ui_text::body(format!(" — {}", rate.footnote_detail()), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_padding_left(2.0)
        .finish(),
    );
    Container::new(line.finish())
        .with_margin_left(10.0)
        .with_margin_right(10.0)
        .with_margin_top(4.0)
        .with_margin_bottom(6.0)
        .with_padding_left(10.0)
        .with_padding_right(10.0)
        .with_padding_top(8.0)
        .with_padding_bottom(8.0)
        .with_background(theme::canvas())
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
}

pub fn render_model_rate_menu(font: FamilyId, rate: AgentModelRate) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
    col.add_child(popover_menu_header(font, "模型倍率"));
    for option in AgentModelRate::all() {
        col.add_child(model_rate_option(font, option, option == rate));
    }
    col.add_child(model_rate_footnote(font, rate));
    popover_shell(MODEL_RATE_POPOVER_WIDTH, col.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_model_chip_label_matches_rate() {
        assert_eq!(
            composer_model_chip_label(AgentModelRate::X03),
            "GPT-5.5 ×0.3"
        );
        assert_eq!(
            composer_model_chip_label(AgentModelRate::X01),
            "GPT-5.5 ×0.1"
        );
        assert_eq!(
            composer_model_chip_label(AgentModelRate::X05),
            "GPT-5.5 ×0.5"
        );
    }
}
