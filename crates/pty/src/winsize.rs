//! Terminal window dimensions in cells and pixels.

/// Terminal window size: grid dimensions plus the pixel size of one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Winsize {
    /// Number of columns in the grid.
    pub cols: u16,
    /// Number of rows in the grid.
    pub rows: u16,
    /// Pixel width of a single cell (0 if unknown).
    pub cell_width: u16,
    /// Pixel height of a single cell (0 if unknown).
    pub cell_height: u16,
}

impl Winsize {
    /// Size with the given grid and unknown cell pixel dimensions.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cell_width: 0,
            cell_height: 0,
        }
    }

    /// Size with grid and per-cell pixel dimensions.
    pub fn with_cell_size(cols: u16, rows: u16, cell_width: u16, cell_height: u16) -> Self {
        Self {
            cols,
            rows,
            cell_width,
            cell_height,
        }
    }

    /// Convert to the kernel `struct winsize`. Pixel fields are the total
    /// window pixel size (grid * cell), saturating on overflow.
    pub fn to_termios(self) -> rustix::termios::Winsize {
        rustix::termios::Winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.cols.saturating_mul(self.cell_width),
            ws_ypixel: self.rows.saturating_mul(self.cell_height),
        }
    }
}

impl Default for Winsize {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn default_is_80x24() {
        let ws = Winsize::default();
        assert_eq!(ws.cols, 80);
        assert_eq!(ws.rows, 24);
        assert_eq!(ws.cell_width, 0);
        assert_eq!(ws.cell_height, 0);
    }

    #[test]
    fn converts_grid_to_termios() {
        let ws = Winsize::new(120, 40).to_termios();
        assert_eq!(ws.ws_col, 120);
        assert_eq!(ws.ws_row, 40);
        assert_eq!(ws.ws_xpixel, 0);
        assert_eq!(ws.ws_ypixel, 0);
    }

    #[test]
    fn converts_pixels_as_total_window_size() {
        let ws = Winsize::with_cell_size(100, 50, 8, 16).to_termios();
        assert_eq!(ws.ws_xpixel, 800);
        assert_eq!(ws.ws_ypixel, 800);
    }

    #[test]
    fn pixel_conversion_saturates() {
        let ws = Winsize::with_cell_size(u16::MAX, u16::MAX, u16::MAX, u16::MAX).to_termios();
        assert_eq!(ws.ws_xpixel, u16::MAX);
        assert_eq!(ws.ws_ypixel, u16::MAX);
    }
}
