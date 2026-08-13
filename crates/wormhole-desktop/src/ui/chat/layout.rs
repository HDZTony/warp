use warpui::elements::{CornerRadius, Radius};

use crate::ui::panel_primitives::{TG_BUBBLE_MAX_WIDTH, TG_BUBBLE_RADIUS, TG_BUBBLE_TAIL_RADIUS};

pub const TG_THREAD_PAD_TOP: f32 = 12.0;
pub const TG_THREAD_PAD_X: f32 = 16.0;
pub const TG_THREAD_PAD_BOTTOM: f32 = 16.0;
pub const TG_MSG_GROUPED_GAP: f32 = 2.0;
pub const TG_MSG_GAP: f32 = 10.0;

pub fn message_is_grouped(prev_outgoing: Option<bool>, outgoing: bool) -> bool {
    prev_outgoing == Some(outgoing)
}

pub fn attachment_preview_width(max_bubble_width: f32) -> f32 {
    max_bubble_width.clamp(160.0, 280.0)
}

pub fn bubble_max_width(parent_width: f32) -> f32 {
    if !parent_width.is_finite() || parent_width <= 0.0 {
        return TG_BUBBLE_MAX_WIDTH;
    }
    (parent_width * 0.72).min(TG_BUBBLE_MAX_WIDTH)
}

pub fn bubble_corner_radius(outgoing: bool, grouped: bool) -> CornerRadius {
    let large = Radius::Pixels(TG_BUBBLE_RADIUS);
    let small = Radius::Pixels(TG_BUBBLE_TAIL_RADIUS);
    let mut radius = CornerRadius::with_all(large);
    if outgoing {
        radius.merge(CornerRadius::with_bottom_right(small));
        if grouped {
            radius.merge(CornerRadius::with_top_right(small));
        }
    } else {
        radius.merge(CornerRadius::with_bottom_left(small));
        if grouped {
            radius.merge(CornerRadius::with_top_left(small));
        }
    }
    radius
}

pub fn message_row_margin_bottom(grouped_with_next: bool) -> f32 {
    if grouped_with_next {
        TG_MSG_GROUPED_GAP
    } else {
        TG_MSG_GAP
    }
}

pub fn thread_search_matches(query: &str, body: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    body.to_lowercase().contains(&query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_only_when_same_direction() {
        assert!(!message_is_grouped(None, true));
        assert!(message_is_grouped(Some(true), true));
        assert!(!message_is_grouped(Some(false), true));
    }

    #[test]
    fn bubble_max_width_caps_at_520() {
        assert_eq!(bubble_max_width(2000.0), 520.0);
        assert!((bubble_max_width(600.0) - 432.0).abs() < 0.01);
    }

    #[test]
    fn attachment_preview_width_is_tight() {
        assert_eq!(attachment_preview_width(520.0), 280.0);
        assert_eq!(attachment_preview_width(200.0), 200.0);
        assert_eq!(attachment_preview_width(80.0), 160.0);
    }

    #[test]
    fn row_gap_switches_between_grouped_and_standalone() {
        assert_eq!(message_row_margin_bottom(true), TG_MSG_GROUPED_GAP);
        assert_eq!(message_row_margin_bottom(false), TG_MSG_GAP);
    }

    #[test]
    fn thread_search_is_case_insensitive() {
        assert!(thread_search_matches("verify", "MCP-RECEIVE-VERIFY"));
        assert!(!thread_search_matches("missing", "hello"));
        assert!(thread_search_matches("", "anything"));
    }
}
