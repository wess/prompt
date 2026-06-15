//! Cursor state: position, pen, pending-wrap, saved cursor, and style.

use crate::cell::Cell;
use crate::charset::Charsets;

/// The live cursor of a screen.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor {
    /// 0-based row in grid coordinates.
    pub row: usize,
    /// 0-based column.
    pub col: usize,
    /// Template cell carrying the current SGR attributes.
    pub pen: Cell,
    /// Set after printing in the last column with autowrap on; the next
    /// printable character wraps to the start of the following line.
    pub pending_wrap: bool,
}

/// State captured by DECSC and restored by DECRC.
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedCursor {
    pub row: usize,
    pub col: usize,
    pub pen: Cell,
    pub charsets: Charsets,
    pub origin: bool,
    pub pending_wrap: bool,
}

/// Cursor shape as selected by DECSCUSR (`CSI Ps SP q`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    #[default]
    BlinkingBlock,
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}

impl CursorStyle {
    /// Style for a DECSCUSR parameter; `None` for out-of-range values.
    pub fn from_decscusr(param: u16) -> Option<CursorStyle> {
        match param {
            0 | 1 => Some(CursorStyle::BlinkingBlock),
            2 => Some(CursorStyle::SteadyBlock),
            3 => Some(CursorStyle::BlinkingUnderline),
            4 => Some(CursorStyle::SteadyUnderline),
            5 => Some(CursorStyle::BlinkingBar),
            6 => Some(CursorStyle::SteadyBar),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_cursor_is_home() {
        let c = Cursor::default();
        assert_eq!((c.row, c.col), (0, 0));
        assert!(!c.pending_wrap);
    }

    #[test]
    fn decscusr_mapping() {
        assert_eq!(CursorStyle::from_decscusr(0), Some(CursorStyle::BlinkingBlock));
        assert_eq!(CursorStyle::from_decscusr(1), Some(CursorStyle::BlinkingBlock));
        assert_eq!(CursorStyle::from_decscusr(2), Some(CursorStyle::SteadyBlock));
        assert_eq!(CursorStyle::from_decscusr(3), Some(CursorStyle::BlinkingUnderline));
        assert_eq!(CursorStyle::from_decscusr(4), Some(CursorStyle::SteadyUnderline));
        assert_eq!(CursorStyle::from_decscusr(5), Some(CursorStyle::BlinkingBar));
        assert_eq!(CursorStyle::from_decscusr(6), Some(CursorStyle::SteadyBar));
        assert_eq!(CursorStyle::from_decscusr(7), None);
    }

    #[test]
    fn saved_cursor_default_is_home() {
        let s = SavedCursor::default();
        assert_eq!((s.row, s.col), (0, 0));
        assert!(!s.origin);
    }
}
