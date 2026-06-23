use pathfinder_color::ColorU;
use warpui::elements::{Container, CrossAxisAlignment, Flex, MainAxisSize, ParentElement};
use warpui::fonts::FamilyId;
use warpui::Element;

use crate::ui::theme;
use crate::ui_text;

#[derive(Debug, Clone)]
pub struct TranscriptLine {
    pub channel: String,
    pub text: String,
    pub level: String,
}

pub fn channel_color(channel: &str, level: &str) -> ColorU {
    if level == "error" || channel == "stderr" {
        return theme::danger();
    }
    if channel == "status" || channel == "assistant" || channel == "user" {
        return theme::accent();
    }
    theme::muted()
}

pub fn render_transcript(
    lines: &[TranscriptLine],
    font: FamilyId,
    mono: FamilyId,
) -> Box<dyn warpui::Element> {
    let mut column = Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Start)
        .with_main_axis_size(MainAxisSize::Min);

    if lines.is_empty() {
        column.add_child(
            ui_text::body("输入消息开始对话，或切换到全自动模式提交任务。", font)
                .with_color(theme::muted())
                .finish(),
        );
    } else {
        for line in lines {
            let channel_label = line.channel.to_uppercase();
            let rendered = format!("[{channel_label}] {}", line.text);
            column.add_child(
                Container::new(
                    ui_text::mono(rendered, mono)
                        .with_color(channel_color(&line.channel, &line.level))
                        .finish(),
                )
                .with_vertical_margin(3.0)
                .finish(),
            );
        }
    }

    column.finish()
}
