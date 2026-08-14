//! Shared cluster topology geometry aligned with `docs/design/desktop-current.html`.

use pathfinder_geometry::vector::{vec2f, Vector2F};

/// Matches HTML `--panel-pad` / `.device-grid` padding.
pub const TOPO_PAD: f32 = 14.0;
/// Matches `.device-grid { gap: 12px }`.
pub const CARD_GAP: f32 = 12.0;
/// Prefer wrapping before cramming five cards into a clipped row.
/// Slightly above HTML `minmax(160px, 1fr)` so typical windows wrap earlier.
pub const CARD_MIN_WIDTH: f32 = 160.0;
/// Vertical scrollbar gutter reserved so the last column is not clipped.
pub const SCROLLBAR_GUTTER: f32 = 14.0;
/// Minimum `.device-body` content height (name + status + share count + 24px action buttons).
/// Kept tight to natural content so the card does not leave empty space under the actions.
pub const BODY_MIN_HEIGHT: f32 = 88.0;
pub const BODY_PADDING_TOP: f32 = 10.0;
pub const BODY_PADDING_BOTTOM: f32 = 10.0;
pub const BODY_VERTICAL_PADDING: f32 = BODY_PADDING_TOP + BODY_PADDING_BOTTOM;
pub const CARD_BORDER: f32 = 1.0;

/// Edge inset factor from HTML `edgePoints` (`min(w,h) * 0.38`).
pub const EDGE_INSET_FACTOR: f32 = 0.38;

pub fn grid_inner_width(container_width: f32) -> f32 {
    (container_width - TOPO_PAD * 2.0 - SCROLLBAR_GUTTER).max(1.0)
}

/// HTML-style `repeat(auto-fill, minmax(CARD_MIN_WIDTH, 1fr))` column count.
pub fn grid_column_count(container_width: f32) -> usize {
    let inner = grid_inner_width(container_width);
    ((inner + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP))
        .floor()
        .max(1.0) as usize
}

pub fn grid_card_width(container_width: f32) -> f32 {
    let columns = grid_column_count(container_width).max(1) as f32;
    let inner = grid_inner_width(container_width);
    let gaps = CARD_GAP * (columns - 1.0).max(0.0);
    // Floor so float rounding never pushes the last column past the clip edge.
    ((inner - gaps) / columns).floor().max(1.0)
}

pub fn thumb_height(card_width: f32) -> f32 {
    card_width * 0.75
}

pub fn card_height(card_width: f32) -> f32 {
    thumb_height(card_width) + BODY_MIN_HEIGHT + BODY_VERTICAL_PADDING + CARD_BORDER * 2.0
}

/// Row/column for a node in an auto-fill grid.
pub fn grid_cell(index: usize, columns: usize) -> (usize, usize) {
    let columns = columns.max(1);
    (index / columns, index % columns)
}

/// Link anchor: card center (multi-row auto-fill) + vertical thumb center.
pub fn node_anchor(count: usize, index: usize, container: Vector2F) -> Vector2F {
    let index = index.min(count.saturating_sub(1));
    let columns = grid_column_count(container.x()).max(1);
    let card_w = grid_card_width(container.x());
    let card_h = card_height(card_w);
    let (row, col) = grid_cell(index, columns);
    let x = TOPO_PAD + col as f32 * (card_w + CARD_GAP) + card_w * 0.5;
    let y = TOPO_PAD + row as f32 * (card_h + CARD_GAP) + thumb_height(card_w) * 0.5;
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
    fn anchors_share_y_for_two_nodes_on_same_row() {
        let size = vec2f(640.0, 280.0);
        let a = node_anchor(2, 0, size);
        let b = node_anchor(2, 1, size);
        assert!((a.y() - b.y()).abs() < f32::EPSILON);
        assert!(a.x() < b.x());
    }

    #[test]
    fn narrow_container_wraps_five_nodes_to_multiple_rows() {
        // One column: 160 + 2*14 pad + gutter; force wrap with 5 nodes.
        let width = 300.0; // inner ≈ 258 → floor((258+12)/(160+12)) = 1
        let columns = grid_column_count(width);
        assert!(columns < 5, "expected wrap, got {columns} columns");
        let size = vec2f(width, 800.0);
        let first = node_anchor(5, 0, size);
        let wrapped = node_anchor(5, columns, size);
        assert!(
            wrapped.y() > first.y() + 1.0,
            "second-row anchor should be below first row: {} vs {}",
            wrapped.y(),
            first.y()
        );
    }

    #[test]
    fn typical_width_wraps_five_cards_before_clipping() {
        // With 160px min width, ~1100px content fits more columns than before but
        // still must not clip the last card in a row.
        let width = 1100.0;
        let columns = grid_column_count(width);
        assert!(
            columns >= 4,
            "expected denser grid with 160px min, got {columns}"
        );
        let total = columns as f32 * grid_card_width(width)
            + (columns.saturating_sub(1) as f32) * CARD_GAP
            + TOPO_PAD * 2.0;
        assert!(
            total + SCROLLBAR_GUTTER <= width + 0.5,
            "row must fit inside container with scrollbar gutter: total={total} width={width}"
        );
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

    #[test]
    fn card_height_includes_body_padding_and_border() {
        let card_w = CARD_MIN_WIDTH;
        assert_eq!(
            card_height(card_w),
            thumb_height(card_w) + BODY_MIN_HEIGHT + BODY_VERTICAL_PADDING + CARD_BORDER * 2.0
        );
    }

    #[test]
    fn row_width_fits_with_trailing_gaps_only() {
        let width = 1100.0;
        let columns = grid_column_count(width).max(1);
        let card_w = grid_card_width(width);
        let row = columns as f32 * card_w
            + (columns.saturating_sub(1) as f32) * CARD_GAP
            + TOPO_PAD * 2.0;
        assert!(
            row + SCROLLBAR_GUTTER <= width + 0.5,
            "trailing-gap-only row must fit: row={row} width={width} cols={columns} card={card_w}"
        );
    }
}
