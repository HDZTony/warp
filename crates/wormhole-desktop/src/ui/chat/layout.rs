use chrono::{Datelike, TimeZone};
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

/// Longest edge / height caps for Telegram-style photo thumbnails in bubbles.
pub const ATTACH_PREVIEW_MAX_EDGE: f32 = 280.0;
pub const ATTACH_PREVIEW_MAX_HEIGHT: f32 = 320.0;

/// Fit source pixel size into the bubble preview box without letterboxing empty
/// space: the ConstrainedBox size equals the fitted size.
pub fn fit_attachment_preview(
    src_w: u32,
    src_h: u32,
    max_w: f32,
    max_h: f32,
) -> (f32, f32) {
    let src_w = src_w.max(1) as f32;
    let src_h = src_h.max(1) as f32;
    let max_w = max_w.max(1.0);
    let max_h = max_h.max(1.0);
    let scale = (max_w / src_w).min(max_h / src_h).min(1.0);
    ((src_w * scale).round().max(1.0), (src_h * scale).round().max(1.0))
}

/// Compact loading chip while image bytes decode — never a fixed 280×210 slab.
pub fn attachment_loading_chip_size(max_bubble_width: f32) -> (f32, f32) {
    let w = attachment_preview_width(max_bubble_width).min(220.0);
    (w, 36.0)
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

/// Local calendar day key (`YYYY-MM-DD`) for date dividers.
pub fn message_local_day_key(timestamp_ms: u64) -> Option<String> {
    let millis = if timestamp_ms == 0 {
        return None;
    } else if timestamp_ms < 1_000_000_000_000 {
        (timestamp_ms as i64) * 1000
    } else {
        timestamp_ms as i64
    };
    let secs = millis.div_euclid(1000);
    let nsec = (millis.rem_euclid(1000) * 1_000_000) as u32;
    chrono::Local
        .timestamp_opt(secs, nsec)
        .single()
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

/// Human label for a day key relative to today (今天 / 昨天 / YYYY/M/D).
pub fn format_date_divider_label(day_key: &str) -> String {
    use chrono::{Duration, Local, NaiveDate};
    let Ok(date) = NaiveDate::parse_from_str(day_key, "%Y-%m-%d") else {
        return day_key.to_string();
    };
    let today = Local::now().date_naive();
    if date == today {
        return "今天".into();
    }
    if date == today - Duration::days(1) {
        return "昨天".into();
    }
    format!("{}/{}/{}", date.year(), date.month(), date.day())
}

/// Whether a new date divider should appear before `current` given previous day key.
pub fn should_insert_date_divider(prev_day: Option<&str>, current_day: Option<&str>) -> bool {
    match (prev_day, current_day) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(prev), Some(cur)) => prev != cur,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Local, NaiveDate};

    #[test]
    fn grouped_only_when_same_direction() {
        assert!(!message_is_grouped(None, true));
        assert!(message_is_grouped(Some(true), true));
        assert!(!message_is_grouped(Some(false), true));
    }

    #[test]
    fn bubble_max_width_caps_at_430() {
        assert_eq!(bubble_max_width(2000.0), 430.0);
        assert!((bubble_max_width(500.0) - 360.0).abs() < 0.01);
    }

    #[test]
    fn attachment_preview_width_is_tight() {
        assert_eq!(attachment_preview_width(430.0), 280.0);
        assert_eq!(attachment_preview_width(200.0), 200.0);
        assert_eq!(attachment_preview_width(80.0), 160.0);
    }

    #[test]
    fn fit_attachment_preview_square() {
        let (w, h) = fit_attachment_preview(800, 800, 280.0, 320.0);
        assert!((w - 280.0).abs() < 0.5);
        assert!((h - 280.0).abs() < 0.5);
    }

    #[test]
    fn fit_attachment_preview_wide() {
        let (w, h) = fit_attachment_preview(1600, 900, 280.0, 320.0);
        assert!((w - 280.0).abs() < 0.5);
        assert!((h - 157.5).abs() < 1.0);
    }

    #[test]
    fn fit_attachment_preview_tall() {
        let (w, h) = fit_attachment_preview(900, 1600, 280.0, 320.0);
        assert!((h - 320.0).abs() < 0.5);
        assert!((w - 180.0).abs() < 1.0);
    }

    #[test]
    fn fit_attachment_preview_small_unchanged() {
        let (w, h) = fit_attachment_preview(120, 80, 280.0, 320.0);
        assert!((w - 120.0).abs() < 0.5);
        assert!((h - 80.0).abs() < 0.5);
    }

    #[test]
    fn loading_chip_is_compact_not_square_slab() {
        let (w, h) = attachment_loading_chip_size(430.0);
        assert!(w <= 220.0);
        assert!((h - 36.0).abs() < 0.5);
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

    #[test]
    fn date_divider_inserts_on_day_change() {
        assert!(should_insert_date_divider(None, Some("2026-08-15")));
        assert!(!should_insert_date_divider(
            Some("2026-08-15"),
            Some("2026-08-15")
        ));
        assert!(should_insert_date_divider(
            Some("2026-08-14"),
            Some("2026-08-15")
        ));
        assert!(!should_insert_date_divider(Some("2026-08-15"), None));
    }

    #[test]
    fn date_divider_label_today_yesterday() {
        let today = Local::now().date_naive();
        let today_key = today.format("%Y-%m-%d").to_string();
        assert_eq!(format_date_divider_label(&today_key), "今天");
        let yesterday = today - chrono::Duration::days(1);
        let y_key = yesterday.format("%Y-%m-%d").to_string();
        assert_eq!(format_date_divider_label(&y_key), "昨天");
        let older = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        assert_eq!(
            format_date_divider_label(&older.format("%Y-%m-%d").to_string()),
            format!("{}/{}/{}", older.year(), older.month(), older.day())
        );
    }
}
