//! Colour helpers: hex ↔ ColorU, relative luminance, contrast warnings.

use pathfinder_color::ColorU;

/// Minimum absolute luminance delta between text and canvas before warning.
pub const CONTRAST_WARN_DELTA: f32 = 0.2;

pub fn color_u_to_hex(c: ColorU) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

pub fn parse_hex(input: &str) -> Option<ColorU> {
    let s = input.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(ColorU::new(r, g, b, 255))
}

/// Mix two opaque colours: `pct` percent of `a` + `(100 - pct)` percent of `b`.
///
/// Used for CSS `color-mix`-style shell surfaces (sidebar, active rows, avatars)
/// so they track the runtime palette instead of hardcoded dark hexes.
pub fn mix_opaque(a: ColorU, b: ColorU, pct: u16) -> ColorU {
    let pct = pct.min(100);
    let rest = 100 - pct;
    ColorU::new(
        ((u16::from(a.r) * pct + u16::from(b.r) * rest) / 100) as u8,
        ((u16::from(a.g) * pct + u16::from(b.g) * rest) / 100) as u8,
        ((u16::from(a.b) * pct + u16::from(b.b) * rest) / 100) as u8,
        255,
    )
}

/// Same hue as `base` with a new alpha (decorative glows / dots / scrims).
pub fn with_alpha(base: ColorU, alpha: u8) -> ColorU {
    ColorU::new(base.r, base.g, base.b, alpha)
}

/// Relative luminance in 0..1 (sRGB, WCAG-ish).
pub fn relative_luminance(c: ColorU) -> f32 {
    fn channel(u: u8) -> f32 {
        let v = u as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

pub fn contrast_delta(a: ColorU, b: ColorU) -> f32 {
    (relative_luminance(a) - relative_luminance(b)).abs()
}

pub fn contrast_warning(text: ColorU, canvas: ColorU) -> bool {
    contrast_delta(text, canvas) < CONTRAST_WARN_DELTA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = ColorU::new(253, 249, 230, 255);
        let hex = color_u_to_hex(c);
        assert_eq!(hex, "#FDF9E6");
        assert_eq!(parse_hex(&hex), Some(c));
        assert_eq!(parse_hex("fdf9e6"), Some(c));
    }

    #[test]
    fn contrast_flags_low_delta() {
        let light = ColorU::new(250, 250, 250, 255);
        let near = ColorU::new(240, 240, 240, 255);
        let dark = ColorU::new(20, 20, 20, 255);
        assert!(contrast_warning(near, light));
        assert!(!contrast_warning(dark, light));
    }

    #[test]
    fn mix_opaque_blends_toward_first() {
        let white = ColorU::new(255, 255, 255, 255);
        let black = ColorU::new(0, 0, 0, 255);
        assert_eq!(mix_opaque(white, black, 100), white);
        assert_eq!(mix_opaque(white, black, 0), black);
        let mid = mix_opaque(white, black, 50);
        assert_eq!(mid, ColorU::new(127, 127, 127, 255));
    }

    #[test]
    fn with_alpha_keeps_rgb() {
        let c = ColorU::new(10, 20, 30, 255);
        assert_eq!(with_alpha(c, 40), ColorU::new(10, 20, 30, 40));
    }
}
