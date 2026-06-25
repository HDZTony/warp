use warpui::elements::{
    Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{Action, Element};

use crate::ui::theme;
use crate::ui_text;

/// Height of the integrated title row (tabs + caption). Must match [`super::app_shell`] layout.
pub const CHROME_ROW_HEIGHT: f32 = 54.0;

const CAPTION_HIT_PADDING: f32 = 12.0;

pub fn caption_buttons<A: Action + Copy + 'static>(
    font: FamilyId,
    buttons: [(&'static str, A); 3],
) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for (label_text, action) in buttons {
        let label = ui_text::body(label_text, font)
            .with_color(theme::text())
            .finish();
        let button = Container::new(
            EventHandler::new(label)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_uniform_padding(CAPTION_HIT_PADDING)
        .finish();
        row.add_child(button);
    }
    row.finish()
}
