//! Multiline draft layout helpers (`desktop-current.html` composer / chat input).

pub const LINE_HEIGHT: f32 = 18.0;
pub const BASE_HEIGHT: f32 = 36.0;
/// Single-line inner height for chat compose (`.chat-compose-input { min-height: 22px }`).
pub const COMPOSE_BASE_HEIGHT: f32 = 22.0;
pub const MAX_LINES: usize = 8;
pub const DEFAULT_COLS: usize = 48;

pub fn visible_line_count(draft: &str, cols: usize) -> usize {
    let cols = cols.max(1);
    if draft.is_empty() {
        return 1;
    }
    let mut lines = 0usize;
    for line in draft.split('\n') {
        let char_count = line.chars().count();
        let wrapped = if char_count == 0 {
            1
        } else {
            char_count.div_ceil(cols)
        };
        lines += wrapped;
    }
    lines.max(1).min(MAX_LINES)
}

pub fn box_height(draft: &str, cols: usize) -> f32 {
    let lines = visible_line_count(draft, cols);
    BASE_HEIGHT + (lines.saturating_sub(1) as f32) * LINE_HEIGHT
}

pub fn compose_box_height(draft: &str, cols: usize) -> f32 {
    let lines = visible_line_count(draft, cols);
    COMPOSE_BASE_HEIGHT + (lines.saturating_sub(1) as f32) * LINE_HEIGHT
}

pub fn display_draft(draft: &str, placeholder: &str) -> String {
    if draft.is_empty() {
        placeholder.to_string()
    } else {
        draft.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{box_height, compose_box_height, visible_line_count};

    #[test]
    fn empty_is_one_line() {
        assert_eq!(visible_line_count("", 40), 1);
        assert_eq!(box_height("", 40), 36.0);
        assert_eq!(compose_box_height("", 40), 22.0);
    }

    #[test]
    fn newline_increases_lines() {
        assert_eq!(visible_line_count("a\nb\nc", 40), 3);
    }

    #[test]
    fn wrap_increases_lines() {
        let long = "x".repeat(100);
        assert!(visible_line_count(&long, 40) >= 3);
    }
}
