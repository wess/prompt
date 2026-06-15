//! Cell metrics: grid <-> pixel conversions for the terminal surface.

/// Pixel size of one terminal cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSize {
    pub width: f32,
    pub height: f32,
}

/// Inner window padding in pixels, applied on every side.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    pub x: f32,
    pub y: f32,
}

/// Line height as a multiple of the font size. Terminal rows are denser
/// than editor lines; 1.3 tracks zed's "standard" terminal line height.
pub const LINE_HEIGHT: f32 = 1.3;

/// Fallback advance-to-font-size ratio when the font cannot be measured.
pub const FALLBACK_ADVANCE: f32 = 0.6;

/// How many whole cells fit in a surface of `width` x `height` pixels.
/// Never returns fewer than 2 columns or 1 row (a 1-column grid breaks
/// wide-character rendering).
pub fn grid_size(width: f32, height: f32, pad: Padding, cell: CellSize) -> (usize, usize) {
    let usable_w = (width - 2.0 * pad.x).max(0.0);
    let usable_h = (height - 2.0 * pad.y).max(0.0);
    let cols = (usable_w / cell.width).floor() as usize;
    let rows = (usable_h / cell.height).floor() as usize;
    (cols.max(2), rows.max(1))
}

/// Pixel size of a window whose content area holds exactly `cols` x `rows`
/// cells plus padding.
pub fn pixel_size(cols: usize, rows: usize, pad: Padding, cell: CellSize) -> (f32, f32) {
    (
        cols as f32 * cell.width + 2.0 * pad.x,
        rows as f32 * cell.height + 2.0 * pad.y,
    )
}

/// Map a window-space position onto a grid cell. `origin` is the element
/// bounds origin; padding is applied here. Positions outside the grid
/// clamp to the nearest cell, so drags past any edge stay valid.
pub fn cell_at(
    pos: (f32, f32),
    origin: (f32, f32),
    pad: Padding,
    cell: CellSize,
    cols: usize,
    rows: usize,
) -> (usize, usize) {
    let col = ((pos.0 - origin.0 - pad.x) / cell.width).floor();
    let row = ((pos.1 - origin.1 - pad.y) / cell.height).floor();
    let col = (col.max(0.0) as usize).min(cols.saturating_sub(1));
    let row = (row.max(0.0) as usize).min(rows.saturating_sub(1));
    (row, col)
}

/// Map a viewport cell to the vt selection coordinate scheme: content line
/// 0 is the top live-grid row, so a viewport rendered at `display_offset`
/// shows line `row - display_offset` (negative lines are scrollback).
pub fn selection_point(row: usize, col: usize, display_offset: usize) -> vt::Point {
    vt::Point::new(row as isize - display_offset as isize, col)
}

/// Measure the cell box for a font: advance width of `M` and the terminal
/// line height. Falls back to a fixed ratio when the glyph is missing.
pub fn measure(text_system: &gpui::TextSystem, font: &gpui::Font, font_size: gpui::Pixels) -> CellSize {
    let font_id = text_system.resolve_font(font);
    let width = text_system
        .advance(font_id, font_size, 'M')
        .map(|advance| f32::from(advance.width))
        .unwrap_or_else(|_| f32::from(font_size) * FALLBACK_ADVANCE);
    CellSize {
        width,
        height: f32::from(font_size) * LINE_HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: CellSize = CellSize {
        width: 8.0,
        height: 17.0,
    };
    const PAD: Padding = Padding { x: 2.0, y: 2.0 };

    #[test]
    fn grid_size_floors_partial_cells() {
        // 644 - 4 = 640 / 8 = 80 cols; 412.8 - 4 = 408.8 / 17 = 24.04 -> 24.
        assert_eq!(grid_size(644.0, 412.8, PAD, CELL), (80, 24));
        // One pixel short of a column.
        assert_eq!(grid_size(643.0, 412.8, PAD, CELL), (79, 24));
    }

    #[test]
    fn grid_size_clamps_minimums() {
        assert_eq!(grid_size(0.0, 0.0, PAD, CELL), (2, 1));
        assert_eq!(grid_size(-50.0, 5.0, PAD, CELL), (2, 1));
        assert_eq!(grid_size(10.0, 18.0, Padding::default(), CELL), (2, 1));
    }

    #[test]
    fn pixel_size_includes_padding_on_both_sides() {
        assert_eq!(pixel_size(80, 24, PAD, CELL), (644.0, 412.0));
        assert_eq!(
            pixel_size(80, 24, Padding::default(), CELL),
            (640.0, 408.0)
        );
    }

    #[test]
    fn round_trips_exact_grids() {
        let (w, h) = pixel_size(120, 40, PAD, CELL);
        assert_eq!(grid_size(w, h, PAD, CELL), (120, 40));
    }

    #[test]
    fn line_height_factor_is_sane() {
        assert!(LINE_HEIGHT > 1.0 && LINE_HEIGHT < 2.0);
    }

    #[test]
    fn cell_at_accounts_for_origin_and_padding() {
        // Window origin (100, 50), pad 2: cell (0,0) spans x 102..110.
        assert_eq!(cell_at((102.0, 52.0), (100.0, 50.0), PAD, CELL, 80, 24), (0, 0));
        assert_eq!(cell_at((109.9, 68.9), (100.0, 50.0), PAD, CELL, 80, 24), (0, 0));
        // One pixel into the next cell each way.
        assert_eq!(cell_at((110.0, 69.0), (100.0, 50.0), PAD, CELL, 80, 24), (1, 1));
        // Mid-grid.
        assert_eq!(cell_at((102.0 + 8.0 * 10.0, 52.0 + 17.0 * 3.0), (100.0, 50.0), PAD, CELL, 80, 24), (3, 10));
    }

    #[test]
    fn cell_at_clamps_to_grid() {
        // Inside the padding band, above/left of cell 0.
        assert_eq!(cell_at((0.0, 0.0), (0.0, 0.0), PAD, CELL, 80, 24), (0, 0));
        // Way past the bottom-right corner.
        assert_eq!(cell_at((9999.0, 9999.0), (0.0, 0.0), PAD, CELL, 80, 24), (23, 79));
        // Negative positions (drag left/above the window).
        assert_eq!(cell_at((-50.0, -50.0), (0.0, 0.0), PAD, CELL, 80, 24), (0, 0));
        // Degenerate grid never underflows.
        assert_eq!(cell_at((5.0, 5.0), (0.0, 0.0), PAD, CELL, 0, 0), (0, 0));
    }

    #[test]
    fn selection_point_maps_display_offset() {
        // Live view: viewport row == content line.
        assert_eq!(selection_point(0, 3, 0), vt::Point::new(0, 3));
        assert_eq!(selection_point(5, 0, 0), vt::Point::new(5, 0));
        // Scrolled back 4 lines: top viewport row shows scrollback line -4.
        assert_eq!(selection_point(0, 1, 4), vt::Point::new(-4, 1));
        assert_eq!(selection_point(4, 1, 4), vt::Point::new(0, 1));
        assert_eq!(selection_point(6, 9, 4), vt::Point::new(2, 9));
    }
}
