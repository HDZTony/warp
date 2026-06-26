use warpui::elements::{
    Border, Container, CrossAxisAlignment, DispatchEventResult, EventHandler, Flex, MainAxisSize,
    ParentElement,
};
use warpui::fonts::FamilyId;
use warpui::{Action, Element};

use crate::ui::theme;
use crate::ui_text;

/// Height of the integrated title row (tabs + caption). Must match [`super::app_shell`] layout.
pub const CHROME_ROW_HEIGHT: f32 = 54.0;

const CAPTION_BTN_WIDTH: f32 = 46.0;
const CAPTION_BTN_HEIGHT: f32 = 38.0;

pub fn caption_buttons<A: Action + Copy + 'static>(
    font: FamilyId,
    buttons: [(&'static str, A); 3],
) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for (glyph, action) in buttons {
        let label = ui_text::caption_glyph(glyph, font)
            .with_color(theme::muted())
            .finish();
        let button = Container::new(
            EventHandler::new(label)
                .on_left_mouse_down(move |ctx, _, _| {
                    ctx.dispatch_typed_action(action);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_horizontal_padding((CAPTION_BTN_WIDTH - 16.0) / 2.0)
        .with_vertical_padding((CAPTION_BTN_HEIGHT - ui_text::CAPTION_GLYPH_SIZE) / 2.0)
        .finish();
        row.add_child(button);
    }
    Container::new(row.finish())
        .with_border(Border::all(1.0).with_border_fill(theme::border()))
        .finish()
}
