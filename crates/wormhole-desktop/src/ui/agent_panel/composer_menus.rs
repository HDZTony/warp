//! Composer access / model popovers — floating HUD menus above the composer bar.

use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::Element;
use wormhole_desktop_core::agent_provider_commands::AgentModelChoiceDto;
use wormhole_desktop_core::warp_embed_prefs::{AgentAccessMode, AgentModelRate};

use super::AgentPanelAction;
use crate::ui::icons;
use crate::ui::panel_primitives::{popover_menu_header, popover_shell_with_radius, section_hint};
use crate::ui::theme;
use crate::ui_text;

pub const ACCESS_POPOVER_WIDTH: f32 = 300.0;
pub const MODEL_RATE_POPOVER_WIDTH: f32 = 320.0;

pub fn access_label(mode: AgentAccessMode) -> &'static str {
    match mode {
        AgentAccessMode::FullAccess => "完全访问",
        AgentAccessMode::WorkspaceWrite => "工作区写入",
    }
}

pub fn composer_model_chip_label(
    choices: &[AgentModelChoiceDto],
    fallback_rate: AgentModelRate,
) -> String {
    if let Some(active) = choices.iter().find(|c| c.is_active) {
        return active.label.clone();
    }
    if let Some(first) = choices.first() {
        return first.label.clone();
    }
    fallback_rate.chip_label().to_string()
}

fn access_option(
    font: FamilyId,
    title: impl Into<String>,
    detail: impl Into<String>,
    icon_path: &'static str,
    selected: bool,
    action: AgentPanelAction,
) -> Box<dyn Element> {
    let title = title.into();
    let detail = detail.into();
    let mut title_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    title_row.add_child(model_radio(selected));
    title_row.add_child(
        Container::new(icons::icon(icon_path, 16.0, theme::muted()))
            .with_padding_left(10.0)
            .finish(),
    );
    title_row.add_child(
        Container::new(
            ui_text::body(title, font)
                .with_color(if selected {
                    theme::text()
                } else {
                    theme::muted()
                })
                .finish(),
        )
        .with_padding_left(8.0)
        .finish(),
    );
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
        "agent-warn.svg",
        mode == AgentAccessMode::FullAccess,
        AgentPanelAction::SelectAccessMode(AgentAccessMode::FullAccess),
    ));
    col.add_child(access_option(
        font,
        access_label(AgentAccessMode::WorkspaceWrite),
        "仅在工作区内写入与修改文件",
        "agent-folder.svg",
        mode == AgentAccessMode::WorkspaceWrite,
        AgentPanelAction::SelectAccessMode(AgentAccessMode::WorkspaceWrite),
    ));
    col.add_child(access_footnote(font, mode));
    popover_shell_with_radius(ACCESS_POPOVER_WIDTH, 14.0, col.finish())
}

fn access_footnote(font: FamilyId, mode: AgentAccessMode) -> Box<dyn Element> {
    let detail = match mode {
        AgentAccessMode::FullAccess => "完全访问会跳过逐项确认，仅在可信任务中使用。",
        AgentAccessMode::WorkspaceWrite => "写入限制在当前项目或工作区内。",
    };
    Container::new(section_hint(detail, font))
        .with_margin_left(10.0)
        .with_margin_right(10.0)
        .with_margin_top(4.0)
        .with_margin_bottom(8.0)
        .with_uniform_padding(10.0)
        .with_background(theme::accent_bg(18))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
        .finish()
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

fn model_choice_option(font: FamilyId, choice: &AgentModelChoiceDto) -> Box<dyn Element> {
    let provider_id = choice.provider_id.clone();
    let model = choice.model.clone();
    let selected = choice.is_active;
    let source_hint = if choice.source == "key_pool" {
        "Key Pool（平台默认）"
    } else {
        "自备 API Key"
    };
    let mut title_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    title_row.add_child(model_radio(selected));
    title_row.add_child(
        Container::new(
            ui_text::body(choice.label.clone(), font)
                .with_color(theme::text())
                .finish(),
        )
        .with_padding_left(10.0)
        .finish(),
    );
    let body = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(title_row.finish())
        .with_child(
            Container::new(section_hint(source_hint, font))
                .with_padding_top(4.0)
                .finish(),
        )
        .finish();

    Container::new(
        EventHandler::new(body)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(AgentPanelAction::SelectAgentModel {
                    provider_id: provider_id.clone(),
                    model: model.clone(),
                });
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

fn model_rate_option(font: FamilyId, rate: AgentModelRate, selected: bool) -> Box<dyn Element> {
    let speed_color = if selected {
        theme::accent()
    } else {
        theme::muted()
    };
    let mut title_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);
    title_row.add_child(model_radio(selected));
    title_row.add_child(
        Container::new(
            ui_text::body(rate.rate_label(), font)
                .with_color(theme::accent_cool())
                .finish(),
        )
        .with_padding_left(10.0)
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

/// Model picker: Key Pool + BYOK choices; Key Pool also keeps rate tiers.
pub fn render_model_menu(
    font: FamilyId,
    choices: &[AgentModelChoiceDto],
    rate: AgentModelRate,
) -> Box<dyn Element> {
    let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
    col.add_child(popover_menu_header(font, "模型"));
    if choices.is_empty() {
        col.add_child(
            Container::new(section_hint(
                "登录后使用 Key Pool，或在设置 → Agent 填写自备 API Key。",
                font,
            ))
            .with_padding_left(14.0)
            .with_padding_right(14.0)
            .with_padding_bottom(10.0)
            .finish(),
        );
    } else {
        for choice in choices {
            col.add_child(model_choice_option(font, choice));
        }
    }

    let key_pool_active = choices
        .iter()
        .any(|c| c.is_active && c.source == "key_pool");
    if key_pool_active || choices.is_empty() {
        col.add_child(popover_menu_header(font, "Key Pool 倍率"));
        for option in AgentModelRate::all() {
            col.add_child(model_rate_option(font, option, option == rate));
        }
    }

    popover_shell_with_radius(MODEL_RATE_POPOVER_WIDTH, 14.0, col.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_model_chip_falls_back_to_rate_label() {
        assert_eq!(
            composer_model_chip_label(&[], AgentModelRate::X03),
            "GPT-5.5 ×0.3"
        );
    }

    #[test]
    fn composer_model_chip_prefers_active_choice() {
        let choices = vec![AgentModelChoiceDto {
            id: "deepseek:deepseek-v4-pro".into(),
            provider_id: "deepseek".into(),
            model: "deepseek-v4-pro".into(),
            label: "DeepSeek · DeepSeek V4 Pro".into(),
            source: "byok".into(),
            is_active: true,
        }];
        assert_eq!(
            composer_model_chip_label(&choices, AgentModelRate::X03),
            "DeepSeek · DeepSeek V4 Pro"
        );
    }
}
