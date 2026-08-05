use pathfinder_color::ColorU;
use warpui::elements::{
    ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container, CornerRadius,
    CrossAxisAlignment, DispatchEventResult, EventHandler, Fill, Flex, MainAxisAlignment,
    MainAxisSize, ParentElement, Radius, ScrollbarWidth,
};
use warpui::fonts::FamilyId;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use crate::ui::panel_primitives::popover_shell_with_radius;
use crate::ui::theme;
use crate::ui_text;

/// Matches `.tg-compose-emoji-panel` in `docs/design/desktop-current.html`.
const EMOJI_POPOVER_WIDTH: f32 = 280.0;
const EMOJI_POPOVER_RADIUS: f32 = 12.0;
const EMOJI_POPOVER_MAX_HEIGHT: f32 = 200.0;
const EMOJI_COLS: usize = 8;
const EMOJI_CELL: f32 = 32.0;
const EMOJI_GLYPH: f32 = 20.0;
const EMOJI_GAP: f32 = 2.0;

/// Fantantonio Emoji-List-Unicode Smileys face groups through robot (U+1F916).
/// Source: https://github.com/Fantantonio/Emoji-List-Unicode (Smileys-and-Emotion → face-* … robot).
///
/// Omitted on purpose (Windows Segoe UI Emoji / cosmic-text shaping):
/// - `🥲` / `🥸` — missing glyphs (empty picker cells)
/// - ZWJ sequences `😶‍🌫️` / `😮‍💨` / `😵‍💫` — fail to compose; bubble shows
///   split faces + fog/dash/spiral (or a white CBDT block) instead of one glyph
const COMMON_EMOJIS: &[&str] = &[
    "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", //
    "🙂", "🙃", "😉", "😊", "😇", "🥰", "😍", "🤩", //
    "😘", "😗", "☺", "😚", "😙", "😋", "😛", "😜", //
    "🤪", "😝", "🤑", "🤗", "🤭", "🤫", "🤔", "🤐", //
    "🤨", "😐", "😑", "😶", "😏", "😒", "🙄", "😬", //
    "🤥", "😌", "😔", "😪", "🤤", "😴", "😷", "🤒", //
    "🤕", "🤢", "🤮", "🤧", "🥵", "🥶", "🥴", "😵", //
    "🤯", "🤠", "🥳", "😎", "🤓", "🧐", "😕", "😟", //
    "🙁", "☹", "😮", "😯", "😲", "😳", "🥺", "😦", //
    "😧", "😨", "😰", "😥", "😢", "😭", "😱", "😖", //
    "😣", "😞", "😓", "😩", "😫", "🥱", "😤", "😡", //
    "😠", "🤬", "😈", "👿", "💀", "☠", "💩", "🤡", //
    "👹", "👺", "👻", "👽", "👾", "🤖",
];

#[derive(Debug, Clone)]
pub enum StickerPickerEvent {
    InsertEmoji(String),
}

#[derive(Debug, Clone)]
pub enum StickerPickerAction {
    InsertEmoji(&'static str),
}

pub struct StickerPickerView {
    emoji_font: FamilyId,
    scroll: ClippedScrollStateHandle,
}

impl StickerPickerView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        Self {
            emoji_font: crate::ui::fonts::load_emoji_font(ctx),
            scroll: ClippedScrollStateHandle::new(),
        }
    }

    fn emoji_cell(&self, emoji: &'static str) -> Box<dyn Element> {
        let glyph = ui_text::chat_avatar_glyph(emoji, self.emoji_font, EMOJI_GLYPH)
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
        .with_automation_label(emoji)
        .with_automation_id(format!("chat:emoji:{emoji}"))
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
        let chunks: Vec<_> = COMMON_EMOJIS.chunks(EMOJI_COLS).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            for emoji in *chunk {
                row.add_child(self.emoji_cell(emoji));
            }
            let mut cell = Container::new(row.finish());
            if i + 1 < chunks.len() {
                cell = cell.with_padding_bottom(EMOJI_GAP);
            }
            col.add_child(cell.finish());
        }
        Container::new(col.finish())
            .with_padding_left(10.0)
            .with_padding_right(10.0)
            .with_padding_top(10.0)
            .with_padding_bottom(10.0)
            .finish()
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
        let scroll = ClippedScrollable::vertical(
            self.scroll.clone(),
            self.emoji_grid(),
            ScrollbarWidth::Auto,
            Fill::None,
            Fill::None,
            Fill::None,
        )
        .finish();
        let body = ConstrainedBox::new(scroll)
            .with_max_height(EMOJI_POPOVER_MAX_HEIGHT)
            .finish();
        popover_shell_with_radius(EMOJI_POPOVER_WIDTH, EMOJI_POPOVER_RADIUS, body)
    }
}

impl TypedActionView for StickerPickerView {
    type Action = StickerPickerAction;

    fn handle_action(&mut self, action: &StickerPickerAction, ctx: &mut ViewContext<Self>) {
        match action {
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
    fn common_emojis_cover_smileys_through_robot() {
        assert!(COMMON_EMOJIS.len() >= 100);
        assert_eq!(COMMON_EMOJIS.first(), Some(&"😀"));
        assert_eq!(COMMON_EMOJIS.last(), Some(&"🤖"));
        assert!(COMMON_EMOJIS.contains(&"🥳"));
        assert!(COMMON_EMOJIS.contains(&"🤖"));
        // Dropped on Windows Segoe UI Emoji (missing glyphs → empty cells):
        assert!(!COMMON_EMOJIS.contains(&"🥲"));
        assert!(!COMMON_EMOJIS.contains(&"🥸"));
        // Dropped: ZWJ sequences that fail to compose (face-in-clouds etc.):
        assert!(!COMMON_EMOJIS.contains(&"😶‍🌫️"));
        assert!(!COMMON_EMOJIS.contains(&"😮‍💨"));
        assert!(!COMMON_EMOJIS.contains(&"😵‍💫"));
        for emoji in COMMON_EMOJIS {
            assert!(
                !emoji.contains('\u{200D}'),
                "picker must not offer ZWJ sequences that split on Windows: {emoji:?}"
            );
        }
    }

    #[test]
    fn common_emojis_are_non_empty_unicode() {
        for emoji in COMMON_EMOJIS {
            assert!(!emoji.is_empty());
            assert!(emoji.chars().count() >= 1);
        }
    }
}
