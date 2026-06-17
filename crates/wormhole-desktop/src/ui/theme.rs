use pathfinder_color::ColorU;

pub fn canvas() -> ColorU {
    ColorU::new(245, 245, 247, 255)
}

pub fn accent() -> ColorU {
    ColorU::new(0, 122, 255, 255)
}

pub fn panel() -> ColorU {
    ColorU::new(255, 255, 255, 255)
}

pub fn border() -> ColorU {
    ColorU::new(210, 210, 215, 255)
}

pub fn text() -> ColorU {
    ColorU::new(28, 28, 30, 255)
}

pub fn muted() -> ColorU {
    ColorU::new(99, 99, 102, 255)
}

pub fn danger() -> ColorU {
    ColorU::new(255, 59, 48, 255)
}

pub fn accent_bg(alpha: u8) -> ColorU {
    ColorU::new(0, 122, 255, alpha)
}
