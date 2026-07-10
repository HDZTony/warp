use warpui::elements::{Container, CrossAxisAlignment, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{popover_menu_header, popover_shell, section_hint};
use crate::ui_text;
use wormhole_desktop_core::sticker_commands::{sticker_list_packs, StickerPackDto};

const STICKER_POPOVER_WIDTH: f32 = 280.0;

pub struct StickerPickerView {
    core: CoreHandle,
    font: FamilyId,
    packs: Vec<StickerPackDto>,
}

impl StickerPickerView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            packs: Vec::new(),
        };
        view.refresh(ctx);
        view
    }

    fn refresh(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move { sticker_list_packs().await },
            |view, output, ctx| {
                match output {
                    Ok(packs) => view.packs = packs,
                    Err(_) => view.packs.clear(),
                }
                ctx.notify();
            },
        );
    }
}

impl Entity for StickerPickerView {
    type Event = ();
}

impl View for StickerPickerView {
    fn ui_name() -> &'static str {
        "StickerPickerView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(popover_menu_header(self.font, "贴纸"));
        for pack in &self.packs {
            col.add_child(
                Container::new(section_hint(
                    format!("{} ({} stickers)", pack.title, pack.stickers.len()),
                    self.font,
                ))
                .with_padding_left(14.0)
                .with_padding_right(14.0)
                .with_padding_top(8.0)
                .with_padding_bottom(8.0)
                .finish(),
            );
        }
        if self.packs.is_empty() {
            col.add_child(
                Container::new(ui_text::body("无贴纸包", self.font).finish())
                    .with_padding_left(14.0)
                    .with_padding_right(14.0)
                    .with_padding_top(8.0)
                    .with_padding_bottom(8.0)
                    .finish(),
            );
        }
        popover_shell(STICKER_POPOVER_WIDTH, col.finish())
    }
}
