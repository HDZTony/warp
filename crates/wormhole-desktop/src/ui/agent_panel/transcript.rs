use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Flex, MainAxisSize, ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::icons;
use crate::ui::panel_primitives::AGENT_THREAD_MAX_WIDTH;
use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub struct TranscriptLine {
    pub channel: String,
    pub text: String,
    pub level: String,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptViewModel {
    pub user_prompt: Option<String>,
    pub status_line: Option<String>,
    pub assistant_body: Option<String>,
    pub thinking: bool,
    pub lines: Vec<TranscriptLine>,
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
    let bubble = Container::new(
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
    .finish();

    let copy_btn = Container::new(
        EventHandler::new(
            ConstrainedBox::new(icons::agent_icon("agent-copy.svg", theme::muted()))
                .with_width(14.0)
                .with_height(14.0)
                .finish(),
        )
        .on_left_mouse_down(|_, _, _| DispatchEventResult::StopPropagation)
        .finish(),
    )
    .with_uniform_padding(4.0)
    .finish();

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::End)
        .with_child(bubble)
        .with_child(
            Container::new(copy_btn)
                .with_margin_top(6.0)
                .finish(),
        )
        .finish()
}

fn render_assistant_demo(font: FamilyId, status: &str, body: &str, thinking: bool) -> Box<dyn Element> {
    let mut col = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(
            ui_text::body(status.to_string(), font)
                .with_color(theme::muted())
                .finish(),
        )
        .with_child(
            Container::new(
                ConstrainedBox::new(Flex::row().finish())
                    .with_height(1.0)
                    .finish(),
            )
            .with_background(theme::border())
            .with_margin_top(12.0)
            .with_margin_bottom(16.0)
            .finish(),
        )
        .with_child(
            ui_text::body(body.to_string(), font)
                .with_color(theme::text())
                .finish(),
        );
    if thinking {
        col.add_child(
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
            .finish(),
        );
    }
    col.finish()
}

fn render_live_line(line: &TranscriptLine, font: FamilyId, mono: FamilyId) -> Box<dyn Element> {
    match line.channel.as_str() {
        "user" => render_user_message(font, &line.text),
        "assistant" | "stdout" | "stderr" | "status" => {
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
            col.add_child(
                ui_text::mono(line.text.clone(), mono)
                    .with_color(channel_color(&line.channel, &line.level))
                    .finish(),
            );
            col.finish()
        }
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

    if let Some(prompt) = &model.user_prompt {
        column.add_child(
            Container::new(render_user_message(font, prompt))
                .with_margin_bottom(28.0)
                .finish(),
        );
        if let Some(status) = &model.status_line {
            let body = model.assistant_body.as_deref().unwrap_or("");
            column.add_child(render_assistant_demo(
                font,
                status,
                body,
                model.thinking,
            ));
        }
    } else if model.lines.is_empty() {
        column.add_child(
            ui_text::body("输入后续修改或追问…", font)
                .with_color(theme::muted())
                .finish(),
        );
    } else {
        for line in &model.lines {
            let bubble = render_live_line(line, font, mono);
            column.add_child(Container::new(bubble).with_vertical_margin(14.0).finish());
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
    fn demo_transcript_has_user_and_status() {
        let model = TranscriptViewModel {
            user_prompt: Some("hello".into()),
            status_line: Some("已运行 1 秒".into()),
            assistant_body: Some("body".into()),
            thinking: true,
            lines: Vec::new(),
        };
        assert!(model.user_prompt.is_some());
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
