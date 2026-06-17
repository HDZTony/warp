use warpui::elements::{Container, Flex, ParentElement};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui_text;
use wormhole_desktop_core::sticker_commands::{sticker_list_packs, StickerPackDto};

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
        let mut col = Flex::column();
        col.add_child(ui_text::body("贴纸", self.font).finish());
        for pack in &self.packs {
            col.add_child(
                ui_text::mono(
                    format!("{} ({} stickers)", pack.title, pack.stickers.len()),
                    self.font,
                )
                .finish(),
            );
        }
        if self.packs.is_empty() {
            col.add_child(ui_text::body("无贴纸包", self.font).finish());
        }
        Container::new(col.finish())
            .with_uniform_padding(4.0)
            .finish()
    }
}
