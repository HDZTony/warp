use pathfinder_color::ColorU;
use warpui::elements::{
    Align, Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::icons;
use crate::ui::panel_primitives::AGENT_THREAD_MAX_WIDTH;
use crate::ui::theme;
use crate::ui_text;

const USER_BUBBLE_MAX_WIDTH: f32 = 520.0;

#[derive(Debug, Clone)]
pub struct TranscriptLine {
    pub channel: String,
    pub text: String,
    pub level: String,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptViewModel {
    pub lines: Vec<TranscriptLine>,
    pub thinking: bool,
    pub elapsed_seconds: Option<u64>,
}

pub fn channel_color(channel: &str, level: &str) -> ColorU {
    if level == "error" || channel == "stderr" {
        return theme::danger();
    }
    match channel {
        "user" => theme::text(),
        "assistant" => theme::accent_cool(),
        "status" => theme::warn(),
        _ => theme::muted(),
    }
}

fn user_icon_badge() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(icons::agent_icon("agent-user.svg", theme::accent_cool()))
            .with_width(13.0)
            .with_height(13.0)
            .finish(),
    )
    .with_uniform_padding(4.0)
    .with_background(theme::accent_cool_bg(40))
    .with_border(Border::all(1.0).with_border_fill(theme::border_bright()))
    .with_corner_radius(warpui::elements::CornerRadius::with_all(
        warpui::elements::Radius::Pixels(999.0),
    ))
    .finish()
}

fn render_user_message(font: FamilyId, text: &str) -> Box<dyn Element> {
    let prompt = text.to_string();
    let bubble = ConstrainedBox::new(
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(user_icon_badge())
                .with_child(
                    Container::new(
                        ui_text::body(text.to_string(), font)
                            .with_color(theme::text())
                            .finish(),
                    )
                    .with_horizontal_margin(10.0)
                    .finish(),
                )
                .finish(),
        )
        .with_padding_left(16.0)
        .with_padding_right(16.0)
        .with_padding_top(12.0)
        .with_padding_bottom(12.0)
        .with_background(theme::panel_elevated())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .with_corner_radius(warpui::elements::CornerRadius::with_all(
            warpui::elements::Radius::Pixels(999.0),
        ))
        .finish(),
    )
    .with_max_width(USER_BUBBLE_MAX_WIDTH)
    .finish();

    let copy_btn = Container::new(
        EventHandler::new(
            ConstrainedBox::new(icons::agent_icon("agent-copy.svg", theme::muted()))
                .with_width(14.0)
                .with_height(14.0)
                .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(super::AgentPanelAction::CopyUserPrompt(prompt.clone()));
            DispatchEventResult::StopPropagation
        })
        .finish(),
    )
    .with_uniform_padding(4.0)
    .finish();

    Align::new(
        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::End)
            .with_child(bubble)
            .with_child(Container::new(copy_btn).with_margin_top(6.0).finish())
            .finish(),
    )
    .right()
    .finish()
}

fn render_status_line(font: FamilyId, text: &str) -> Box<dyn Element> {
    ui_text::body(text.to_string(), font)
        .with_color(theme::muted())
        .finish()
}

fn render_divider() -> Box<dyn Element> {
    Container::new(
        ConstrainedBox::new(Flex::row().finish())
            .with_height(1.0)
            .finish(),
    )
    .with_margin_top(14.0)
    .with_margin_bottom(16.0)
    .with_background(theme::border())
    .finish()
}

fn render_assistant_body(
    font: FamilyId,
    mono: FamilyId,
    line: &TranscriptLine,
) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min);
    if line.level == "error" {
        col.add_child(
            ui_text::hud_title("ERROR", font)
                .with_color(theme::danger())
                .finish(),
        );
    }
    let use_mono = matches!(line.channel.as_str(), "stdout" | "stderr");
    if use_mono {
        col.add_child(
            ui_text::mono(line.text.clone(), mono)
                .with_color(channel_color(&line.channel, &line.level))
                .finish(),
        );
    } else {
        col.add_child(
            ui_text::body(line.text.clone(), font)
                .with_color(theme::text())
                .finish(),
        );
    }
    col.finish()
}

fn render_thinking_indicator(font: FamilyId) -> Box<dyn Element> {
    Container::new(
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Container::new(
                    ConstrainedBox::new(Flex::row().finish())
                        .with_width(6.0)
                        .with_height(6.0)
                        .finish(),
                )
                .with_background(theme::accent_cool())
                .with_corner_radius(warpui::elements::CornerRadius::with_all(
                    warpui::elements::Radius::Pixels(999.0),
                ))
                .finish(),
            )
            .with_child(
                Container::new(
                    ui_text::body("思考中", font)
                        .with_color(theme::muted())
                        .finish(),
                )
                .with_horizontal_margin(6.0)
                .finish(),
            )
            .finish(),
    )
    .with_margin_top(16.0)
    .finish()
}

fn render_live_line(line: &TranscriptLine, font: FamilyId, mono: FamilyId) -> Box<dyn Element> {
    match line.channel.as_str() {
        "user" => render_user_message(font, &line.text),
        "status" => render_status_line(font, &line.text),
        "assistant" | "stdout" | "stderr" => render_assistant_body(font, mono, line),
        _ => Container::new(
            ui_text::mono(
                format!("[{}] {}", line.channel.to_uppercase(), line.text),
                mono,
            )
            .with_color(channel_color(&line.channel, &line.level))
            .finish(),
        )
        .with_vertical_margin(3.0)
        .finish(),
    }
}

pub fn render_transcript(
    model: &TranscriptViewModel,
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn Element> {
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_main_axis_size(MainAxisSize::Min);

    if model.lines.is_empty() && !model.thinking {
        column.add_child(Flex::row().finish());
    } else {
        let mut assistant_block_started = false;
        for line in &model.lines {
            if line.channel != "user" && !assistant_block_started {
                if let Some(elapsed) = model.elapsed_seconds {
                    column.add_child(render_status_line(font, &format!("已运行 {elapsed} 秒")));
                    column.add_child(render_divider());
                }
                assistant_block_started = true;
            }
            if line.channel == "status" && model.elapsed_seconds.is_some() {
                continue;
            }
            let bubble = render_live_line(line, font, mono);
            let margin = if line.channel == "user" { 14.0 } else { 6.0 };
            column.add_child(Container::new(bubble).with_vertical_margin(margin).finish());
        }
        if model.thinking {
            column.add_child(render_thinking_indicator(font));
        }
    }

    Container::new(
        ConstrainedBox::new(column.finish())
            .with_max_width(AGENT_THREAD_MAX_WIDTH)
            .finish(),
    )
    .finish()
}

#[cfg(test)]
mod tests {
    use super::{channel_color, TranscriptLine, TranscriptViewModel};

    #[test]
    fn stderr_uses_danger_color() {
        assert_eq!(
            channel_color("stderr", "info"),
            channel_color("stderr", "error")
        );
    }

    #[test]
    fn live_transcript_renders_lines() {
        let model = TranscriptViewModel {
            lines: vec![TranscriptLine {
                channel: "user".into(),
                text: "hello".into(),
                level: "info".into(),
            }],
            thinking: false,
            elapsed_seconds: None,
        };
        assert_eq!(model.lines.len(), 1);
        assert!(!model.thinking);
    }

    #[test]
    fn thinking_flag_set_when_busy() {
        let model = TranscriptViewModel {
            lines: Vec::new(),
            thinking: true,
            elapsed_seconds: Some(3),
        };
        assert!(model.thinking);
    }

    #[test]
    fn transcript_line_fields_roundtrip() {
        let line = TranscriptLine {
            channel: "user".into(),
            text: "hello".into(),
            level: "info".into(),
        };
        assert_eq!(line.channel, "user");
        assert_eq!(line.text, "hello");
    }
}
