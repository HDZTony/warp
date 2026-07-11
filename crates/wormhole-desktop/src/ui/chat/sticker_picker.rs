use pathfinder_color::ColorU;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult, EventHandler,
    Flex, MainAxisAlignment, MainAxisSize, ParentElement, Radius,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::core_handle::CoreHandle;
use crate::ui::panel_primitives::{popover_menu_header, popover_shell, section_hint};
use crate::ui::theme;
use crate::ui_text;
use wormhole_desktop_core::sticker_commands::{sticker_list_packs, StickerPackDto};

const STICKER_POPOVER_WIDTH: f32 = 296.0;
const EMOJI_COLS: usize = 8;
const EMOJI_CELL: f32 = 34.0;
const EMOJI_GLYPH: f32 = 20.0;
const TAB_HEIGHT: f32 = 32.0;

/// Common Unicode emoji for the chat picker (system fonts; no bundled assets).
const COMMON_EMOJIS: &[&str] = &[
    "😀", "😁", "😂", "🤣", "😊", "😍", "🤩", "😘", //
    "😉", "😎", "🤔", "😐", "😴", "😢", "😭", "😤", //
    "😡", "🤯", "🥳", "😇", "👍", "👎", "👏", "🙏", //
    "💪", "✌️", "🤝", "👋", "👌", "🤞", "❤️", "🧡", //
    "💛", "💚", "💙", "💜", "🖤", "💔", "💕", "💖", //
    "🔥", "⭐", "✨", "🎉", "🎊", "💯", "✅", "❌", //
    "🚀", "👀", "💀", "💩", "🙈", "🙉", "🙊", "🍀",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerTab {
    Emoji,
    Stickers,
}

#[derive(Debug, Clone)]
pub enum StickerPickerEvent {
    InsertEmoji(String),
}

#[derive(Debug, Clone)]
pub enum StickerPickerAction {
    SelectTab(PickerTab),
    InsertEmoji(&'static str),
}

pub struct StickerPickerView {
    core: CoreHandle,
    font: FamilyId,
    tab: PickerTab,
    packs: Vec<StickerPackDto>,
}

impl StickerPickerView {
    pub fn new(ctx: &mut ViewContext<Self>, core: CoreHandle) -> Self {
        let font = crate::ui::fonts::load_ui_font(ctx);
        let mut view = Self {
            core,
            font,
            tab: PickerTab::Emoji,
            packs: Vec::new(),
        };
        view.refresh_packs(ctx);
        view
    }

    fn refresh_packs(&mut self, ctx: &mut ViewContext<Self>) {
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

    fn tab_btn(&self, label: &str, tab: PickerTab) -> Box<dyn Element> {
        let active = self.tab == tab;
        let (fg, bg) = if active {
            (theme::text(), theme::accent_bg(40))
        } else {
            (theme::muted(), ColorU::transparent_black())
        };
        let label = label.to_string();
        let inner = EventHandler::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(
                        ui_text::body(label, self.font)
                            .with_color(fg)
                            .finish(),
                    )
                    .finish(),
            )
            .with_background(bg)
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .with_padding_left(10.0)
            .with_padding_right(10.0)
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(StickerPickerAction::SelectTab(tab));
            DispatchEventResult::StopPropagation
        })
        .finish();

        ConstrainedBox::new(inner)
            .with_height(TAB_HEIGHT)
            .finish()
    }

    fn tab_bar(&self) -> Box<dyn Element> {
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_child(self.tab_btn("表情", PickerTab::Emoji))
                .with_child(
                    Container::new(self.tab_btn("贴纸", PickerTab::Stickers))
                        .with_margin_left(6.0)
                        .finish(),
                )
                .finish(),
        )
        .with_padding_left(12.0)
        .with_padding_right(12.0)
        .with_padding_bottom(4.0)
        .finish()
    }

    fn emoji_cell(&self, emoji: &'static str) -> Box<dyn Element> {
        let glyph = ui_text::chat_avatar_glyph(emoji, self.font, EMOJI_GLYPH)
            .with_color(theme::text())
            .finish();
        let inner = EventHandler::new(
            Container::new(
                Flex::row()
                    .with_main_axis_alignment(MainAxisAlignment::Center)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_main_axis_size(MainAxisSize::Max)
                    .with_child(glyph)
                    .finish(),
            )
            .with_background(ColorU::transparent_black())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.0)))
            .finish(),
        )
        .on_left_mouse_down(move |ctx, _, _| {
            ctx.dispatch_typed_action(StickerPickerAction::InsertEmoji(emoji));
            DispatchEventResult::StopPropagation
        })
        .finish();

        ConstrainedBox::new(inner)
            .with_width(EMOJI_CELL)
            .with_height(EMOJI_CELL)
            .finish()
    }

    fn emoji_grid(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);
        for chunk in COMMON_EMOJIS.chunks(EMOJI_COLS) {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            for emoji in chunk {
                row.add_child(self.emoji_cell(emoji));
            }
            col.add_child(
                Container::new(row.finish())
                    .with_padding_bottom(2.0)
                    .finish(),
            );
        }
        Container::new(col.finish())
            .with_padding_left(10.0)
            .with_padding_right(10.0)
            .with_padding_bottom(10.0)
            .finish()
    }

    fn stickers_body(&self) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
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
        col.finish()
    }
}

impl Entity for StickerPickerView {
    type Event = StickerPickerEvent;
}

impl View for StickerPickerView {
    fn ui_name() -> &'static str {
        "StickerPickerView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let mut col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        col.add_child(popover_menu_header(self.font, "表情与贴纸"));
        col.add_child(self.tab_bar());
        col.add_child(
            Container::new(
                ConstrainedBox::new(Flex::row().finish())
                    .with_height(1.0)
                    .finish(),
            )
            .with_background(theme::border())
            .with_margin_left(12.0)
            .with_margin_right(12.0)
            .with_margin_bottom(6.0)
            .finish(),
        );
        match self.tab {
            PickerTab::Emoji => col.add_child(self.emoji_grid()),
            PickerTab::Stickers => col.add_child(self.stickers_body()),
        }
        popover_shell(STICKER_POPOVER_WIDTH, col.finish())
    }
}

impl TypedActionView for StickerPickerView {
    type Action = StickerPickerAction;

    fn handle_action(&mut self, action: &StickerPickerAction, ctx: &mut ViewContext<Self>) {
        match action {
            StickerPickerAction::SelectTab(tab) => {
                self.tab = *tab;
                if *tab == PickerTab::Stickers && self.packs.is_empty() {
                    self.refresh_packs(ctx);
                }
                ctx.notify();
            }
            StickerPickerAction::InsertEmoji(emoji) => {
                ctx.emit(StickerPickerEvent::InsertEmoji((*emoji).to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_emojis_fill_complete_rows() {
        assert!(!COMMON_EMOJIS.is_empty());
        assert_eq!(COMMON_EMOJIS.len() % EMOJI_COLS, 0);
    }

    #[test]
    fn common_emojis_are_non_empty_unicode() {
        for emoji in COMMON_EMOJIS {
            assert!(!emoji.is_empty());
            assert!(emoji.chars().count() >= 1);
        }
    }
}
