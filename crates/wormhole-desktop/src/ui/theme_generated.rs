// GENERATED — do not edit. Source: assets/brand-spec.md
// Regenerate: uv run scripts/sync-brand-tokens.py
use pathfinder_color::ColorU;

pub fn canvas() -> ColorU {
    ColorU::new(8, 7, 11, 255)
}

pub fn bg() -> ColorU {
    ColorU::new(15, 18, 35, 255)
}

pub fn accent() -> ColorU {
    ColorU::new(253, 249, 230, 255)
}

pub fn accent_cool() -> ColorU {
    ColorU::new(222, 231, 247, 255)
}

pub fn panel() -> ColorU {
    ColorU::new(42, 39, 54, 255)
}

pub fn border() -> ColorU {
    ColorU::new(100, 93, 117, 255)
}

pub fn text() -> ColorU {
    ColorU::new(245, 231, 207, 255)
}

pub fn muted() -> ColorU {
    ColorU::new(160, 135, 140, 255)
}

pub fn placeholder() -> ColorU {
    ColorU::new(215, 203, 207, 255)
}

pub fn danger() -> ColorU {
    ColorU::new(232, 93, 76, 255)
}

pub fn success() -> ColorU {
    ColorU::new(91, 196, 122, 255)
}

pub fn warn() -> ColorU {
    ColorU::new(229, 199, 107, 255)
}

pub fn accent_bg(alpha: u8) -> ColorU {
    ColorU::new(253, 249, 230, alpha)
}


pub fn accent_bg_default() -> ColorU {
    accent_bg(40)
}
