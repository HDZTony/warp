// GENERATED — do not edit. Source: assets/brand-spec.md
// Regenerate: uv run scripts/sync-brand-tokens.py

use pathfinder_color::ColorU;
use warp_core::ui::theme::color::CustomDetails;
use warp_core::ui::theme::{
    AnsiColor, AnsiColors, Details, Fill, TerminalColors, WarpTheme,
};

const WORMHOLE_NORMAL: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0xA0878CFF),
    AnsiColor::from_u32(0xE85D4CFF),
    AnsiColor::from_u32(0x5BC47AFF),
    AnsiColor::from_u32(0xE5C76BFF),
    AnsiColor::from_u32(0xDEE7F7FF),
    AnsiColor::from_u32(0xFDF9E6FF),
    AnsiColor::from_u32(0xDEE7F7FF),
    AnsiColor::from_u32(0xF5E7CFFF),
);
const WORMHOLE_BRIGHT: AnsiColors = AnsiColors::new(
    AnsiColor::from_u32(0x645D75FF),
    AnsiColor::from_u32(0xE85D4CFF),
    AnsiColor::from_u32(0x5BC47AFF),
    AnsiColor::from_u32(0xE5C76BFF),
    AnsiColor::from_u32(0xDEE7F7FF),
    AnsiColor::from_u32(0xFDF9E6FF),
    AnsiColor::from_u32(0xDEE7F7FF),
    AnsiColor::from_u32(0xF5E7CFFF),
);

fn wormhole_details() -> Details {
    let mut details = CustomDetails::darker_details();
    details.background = Fill::Solid(ColorU::from_u32(0x2A2736FF));
    details.border = Fill::Solid(ColorU::from_u32(0x645D75FF));
    Details::Custom(details)
}

pub fn wormhole_theme() -> WarpTheme {
    WarpTheme::new(
        Fill::Solid(ColorU::from_u32(0x08070BFF)),
        ColorU::from_u32(0xF5E7CFFF),
        Fill::Solid(ColorU::from_u32(0xFDF9E6FF)),
        Some(Fill::Solid(ColorU::from_u32(0xDEE7F7FF))),
        Some(wormhole_details()),
        TerminalColors::new(WORMHOLE_NORMAL, WORMHOLE_BRIGHT),
        None,
        Some("Wormhole".to_string()),
    )
}
