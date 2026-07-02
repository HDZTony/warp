//! Shared cluster topology geometry aligned with `docs/design/desktop-current.html`.

use pathfinder_geometry::vector::{vec2f, Vector2F};

/// Matches HTML `--panel-pad` / `.device-grid` padding.
pub const TOPO_PAD: f32 = 14.0;
/// Matches `.device-grid { gap: 12px }`.
pub const CARD_GAP: f32 = 12.0;
/// Matches `minmax(200px, 1fr)`.
pub const CARD_MIN_WIDTH: f32 = 200.0;
/// Minimum `.device-body` height (name + status + node action buttons).
pub const BODY_MIN_HEIGHT: f32 = 136.0;
pub const CARD_BORDER: f32 = 1.0;

/// Edge inset factor from HTML `edgePoints` (`min(w,h) * 0.38`).
pub const EDGE_INSET_FACTOR: f32 = 0.38;

pub fn grid_inner_width(container_width: f32) -> f32 {
    (container_width - TOPO_PAD * 2.0).max(1.0)
}

/// HTML `repeat(auto-fill, minmax(200px, 1fr))` column count.
pub fn grid_column_count(container_width: f32) -> usize {
    let inner = grid_inner_width(container_width);
    ((inner + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP))
        .floor()
        .max(1.0) as usize
}

pub fn grid_card_width(container_width: f32) -> f32 {
    let columns = grid_column_count(container_width).max(1) as f32;
    let inner = grid_inner_width(container_width);
    (inner - CARD_GAP * (columns - 1.0).max(0.0)) / columns
}

pub fn thumb_height(card_width: f32) -> f32 {
    card_width * 0.75
}

pub fn card_height(card_width: f32) -> f32 {
    thumb_height(card_width) + BODY_MIN_HEIGHT + CARD_BORDER * 2.0
}

pub fn cards_row_card_width(container_width: f32, node_count: usize) -> f32 {
    let cols = grid_column_count(container_width).max(node_count.max(1)) as f32;
    let inner = grid_inner_width(container_width);
    (inner - CARD_GAP * (cols - 1.0).max(0.0)) / cols
}

/// Link anchor: horizontal card center + vertical thumb center.
pub fn node_anchor(count: usize, index: usize, container: Vector2F) -> Vector2F {
    let index = index.min(count.saturating_sub(1));
    let card_w = cards_row_card_width(container.x(), count.max(1));
    let x = TOPO_PAD + index as f32 * (card_w + CARD_GAP) + card_w * 0.5;
    let y = TOPO_PAD + thumb_height(card_w) * 0.5;
    vec2f(x, y)
}

pub fn node_anchors(count: usize, container: Vector2F) -> Vec<Vector2F> {
    (0..count.max(1))
        .map(|i| node_anchor(count.max(1), i, container))
        .collect()
}

/// HTML `edgePoints`: curve endpoints inset from card centers toward each other.
pub fn edge_points(from: Vector2F, to: Vector2F, card_w: f32, card_h: f32) -> (Vector2F, Vector2F) {
    let delta = to - from;
    let len = delta.length().max(1.0);
    let ux = delta.x() / len;
    let uy = delta.y() / len;
    let from_r = card_w.min(card_h) * EDGE_INSET_FACTOR;
    let to_r = card_w.min(card_h) * EDGE_INSET_FACTOR;
    (
        vec2f(from.x() + ux * from_r, from.y() + uy * from_r),
        vec2f(to.x() - ux * to_r, to.y() - uy * to_r),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_share_y_for_two_nodes() {
        let size = vec2f(640.0, 280.0);
        let a = node_anchor(2, 0, size);
        let b = node_anchor(2, 1, size);
        assert!((a.y() - b.y()).abs() < f32::EPSILON);
        assert!(a.x() < b.x());
    }

    #[test]
    fn wide_container_keeps_cards_narrower_than_half_width() {
        let card_w = grid_card_width(1200.0);
        assert!(card_w < 300.0);
        assert!(card_w >= CARD_MIN_WIDTH);
    }

    #[test]
    fn edge_points_are_horizontal_for_pair() {
        let card_w = grid_card_width(640.0);
        let y = TOPO_PAD + thumb_height(card_w) * 0.5;
        let from = vec2f(100.0, y);
        let to = vec2f(400.0, y);
        let (p0, p1) = edge_points(from, to, card_w, card_height(card_w));
        assert!((p0.y() - y).abs() < 0.01);
        assert!((p1.y() - y).abs() < 0.01);
    }
}
