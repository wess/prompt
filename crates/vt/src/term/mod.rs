//! The terminal: owns both screens, modes, and the escape-sequence parser.

mod csi;
mod dcs;
mod ops;
mod osc;
mod perform;
pub mod report;
mod select;

pub use report::{Clipboard, ReportColors};

use crate::cell::Cell;
use crate::charset::Charsets;
use crate::cursor::CursorStyle;
use crate::grid::damage::Damage;
use crate::grid::row::Row;
use crate::grid::Grid;
use crate::hyperlink::{Hyperlink, HyperlinkId, Hyperlinks};
use crate::mode::{Modes, MouseMode};
use crate::screen::Screen;
use crate::selection::Selection;

/// Full terminal state. Feed pty bytes with [`Terminal::feed`], drain
/// responses for the pty with [`Terminal::take_output`], and read cells via
/// the grid/row accessors when rendering.
pub struct Terminal {
    parser: vte::Parser,
    inner: Inner,
}

/// All mutable terminal state; the `vte::Perform` target. Lives apart from
/// the parser because `Parser::advance` borrows both mutably.
pub(crate) struct Inner {
    pub(crate) primary: Screen,
    pub(crate) alt: Screen,
    pub(crate) modes: Modes,
    pub(crate) charsets: Charsets,
    pub(crate) title: String,
    pub(crate) title_stack: Vec<String>,
    pub(crate) cwd: Option<String>,
    /// OSC 4 palette overrides; `None` means "use the theme".
    pub(crate) palette: [Option<(u8, u8, u8)>; 256],
    pub(crate) cursor_color: Option<(u8, u8, u8)>,
    pub(crate) cursor_style: CursorStyle,
    /// Bytes the host must write back to the pty (DSR replies, DA, ...).
    pub(crate) output: Vec<u8>,
    pub(crate) bell: bool,
    /// Set by whole-terminal render events (alt switch, RIS, palette OSC,
    /// resize, scroll-offset changes); overrides per-row grid damage.
    pub(crate) full_damage: bool,
    /// Set when the title changes (OSC 0/2 or XTWINOPS title pop).
    pub(crate) title_changed: bool,
    pub(crate) last_printed: Option<char>,
    /// Lines scrolled back into history for display; 0 = bottom.
    pub(crate) display_offset: usize,
    pub(crate) scrollback_limit: usize,
    /// Active selection, in content-anchored absolute coordinates
    /// (see [`crate::selection`]).
    pub(crate) selection: Option<Selection>,
    /// Extra characters (beyond alphanumerics) word selection treats as
    /// word constituents.
    pub(crate) word_chars: Vec<char>,
    /// Pending OSC 52 clipboard write for the host to act on.
    pub(crate) clipboard: Option<report::Clipboard>,
    /// Colors the host installed for answering OSC color queries.
    pub(crate) report_colors: Option<Box<report::ReportColors>>,
    /// Interned OSC 8 hyperlinks referenced by cells.
    pub(crate) hyperlinks: Hyperlinks,
    /// In-progress device control string (XTGETTCAP), if any.
    pub(crate) dcs: dcs::Dcs,
}

impl Inner {
    pub(crate) fn screen(&self) -> &Screen {
        if self.modes.contains(Modes::ALT_SCREEN) {
            &self.alt
        } else {
            &self.primary
        }
    }

    pub(crate) fn screen_mut(&mut self) -> &mut Screen {
        if self.modes.contains(Modes::ALT_SCREEN) {
            &mut self.alt
        } else {
            &mut self.primary
        }
    }
}

impl Terminal {
    /// A `cols` x `rows` terminal whose primary screen keeps up to
    /// `scrollback_limit` history rows (see [`crate::DEFAULT_SCROLLBACK`]).
    pub fn new(cols: usize, rows: usize, scrollback_limit: usize) -> Terminal {
        Terminal {
            parser: vte::Parser::new(),
            inner: Inner {
                primary: Screen::new(cols, rows, scrollback_limit),
                alt: Screen::new(cols, rows, 0),
                modes: Modes::default(),
                charsets: Charsets::default(),
                title: String::new(),
                title_stack: Vec::new(),
                cwd: None,
                palette: [None; 256],
                cursor_color: None,
                cursor_style: CursorStyle::default(),
                output: Vec::new(),
                bell: false,
                full_damage: true,
                title_changed: false,
                last_printed: None,
                display_offset: 0,
                scrollback_limit,
                selection: None,
                word_chars: vec!['/', '-', '_', '.', '~'],
                clipboard: None,
                report_colors: None,
                hyperlinks: Hyperlinks::default(),
                dcs: dcs::Dcs::None,
            },
        }
    }

    /// Drive the parser with bytes read from the pty.
    pub fn feed(&mut self, bytes: &[u8]) {
        // vte 0.13's `advance` takes a single byte.
        for &byte in bytes {
            self.parser.advance(&mut self.inner, byte);
        }
    }

    /// Simple resize; clamps cursors, resets scroll regions, and drops any
    /// selection (no reflow yet, so old coordinates would lie).
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.inner.primary.resize(cols, rows);
        self.inner.alt.resize(cols, rows);
        self.inner.display_offset = self
            .inner
            .display_offset
            .min(self.inner.primary.grid.scrollback().len());
        self.inner.selection = None;
        self.inner.full_damage = true;
    }

    pub fn cols(&self) -> usize {
        self.inner.screen().grid.cols()
    }

    pub fn rows(&self) -> usize {
        self.inner.screen().grid.rows()
    }

    /// The active screen (alternate when it is enabled).
    pub fn screen(&self) -> &Screen {
        self.inner.screen()
    }

    /// The active screen's grid.
    pub fn grid(&self) -> &Grid {
        &self.inner.screen().grid
    }

    /// Cell accessor on the active grid (no scrollback offset applied).
    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        self.inner.screen().grid.cell(row, col)
    }

    /// Text of a visible row (offset-aware), trimmed; for tests/debugging.
    pub fn row_text(&self, row: usize) -> String {
        self.visible_row(row).text()
    }

    /// Viewport row `i` (0 = top) honoring the display offset into
    /// scrollback. The alternate screen has no scrollback, so the offset
    /// only matters on primary.
    pub fn visible_row(&self, i: usize) -> &Row {
        let grid = &self.inner.screen().grid;
        let sb = grid.scrollback();
        let offset = self.inner.display_offset.min(sb.len());
        let global = sb.len() - offset + i;
        if global < sb.len() {
            sb.get(global).expect("in range")
        } else {
            grid.row(global - sb.len())
        }
    }

    /// All viewport rows, top to bottom, honoring the display offset.
    pub fn visible_rows(&self) -> impl Iterator<Item = &Row> + '_ {
        (0..self.rows()).map(move |i| self.visible_row(i))
    }

    /// How far the view is scrolled back into history (0 = live bottom).
    pub fn display_offset(&self) -> usize {
        self.inner.display_offset
    }

    /// Scroll the view; clamped to available scrollback. Changing the
    /// offset shifts every visible row, so it escalates to full damage.
    /// vt never resets the offset on output by itself (the app decides);
    /// it only keeps it stable as new lines enter scrollback, and resets
    /// it when the alternate screen is entered (no scrollback there).
    pub fn set_display_offset(&mut self, offset: usize) {
        let max = self.inner.screen().grid.scrollback().len();
        let offset = offset.min(max);
        if offset != self.inner.display_offset {
            self.inner.display_offset = offset;
            self.inner.full_damage = true;
        }
    }

    /// Scroll the view by `delta` lines: positive scrolls back into
    /// history, negative toward the live bottom. Clamped to
    /// `[0, scrollback len]`.
    pub fn scroll_display(&mut self, delta: isize) {
        let max = self.inner.screen().grid.scrollback().len() as isize;
        let next = (self.inner.display_offset as isize + delta).clamp(0, max);
        self.set_display_offset(next as usize);
    }

    pub fn title(&self) -> &str {
        &self.inner.title
    }

    pub fn cwd(&self) -> Option<&str> {
        self.inner.cwd.as_deref()
    }

    /// Drain bytes that must be written back to the pty.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.inner.output)
    }

    /// `true` once if a BEL was seen since the last call.
    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.inner.bell)
    }

    /// Accumulated render damage; returns it and resets to clean. A fresh
    /// terminal reports [`Damage::Full`]. Whole-terminal events (resize,
    /// alt-screen switch, RIS, ED 2/3, palette OSCs, display-offset
    /// changes) escalate to full damage.
    pub fn take_damage(&mut self) -> Damage {
        if std::mem::take(&mut self.inner.full_damage) {
            // Everything repaints; drop stale per-row damage on both grids.
            self.inner.primary.grid.take_damage();
            self.inner.alt.grid.take_damage();
            return Damage::Full;
        }
        self.inner.screen_mut().grid.take_damage()
    }

    /// The new title once after it changed (OSC 0/2 or title-stack pop);
    /// `None` until the next change. [`Terminal::title`] always works.
    pub fn take_title_changed(&mut self) -> Option<String> {
        std::mem::take(&mut self.inner.title_changed).then(|| self.inner.title.clone())
    }

    pub fn is_alt_screen(&self) -> bool {
        self.inner.modes.contains(Modes::ALT_SCREEN)
    }

    pub fn cursor_visible(&self) -> bool {
        self.inner.modes.contains(Modes::CURSOR_VISIBLE)
    }

    /// Cursor `(row, col)`, 0-based, in active-grid coordinates.
    pub fn cursor_pos(&self) -> (usize, usize) {
        let c = &self.inner.screen().cursor;
        (c.row, c.col)
    }

    pub fn cursor_style(&self) -> CursorStyle {
        self.inner.cursor_style
    }

    pub fn modes(&self) -> Modes {
        self.inner.modes
    }

    /// DECCKM (CSI ? 1 h/l): application cursor keys.
    pub fn cursor_keys_app(&self) -> bool {
        self.inner.modes.contains(Modes::APP_CURSOR)
    }

    /// DECKPAM/DECKPNM (ESC = / ESC >): application keypad.
    pub fn keypad_app(&self) -> bool {
        self.inner.modes.contains(Modes::APP_KEYPAD)
    }

    /// Bracketed paste (CSI ? 2004 h/l).
    pub fn bracketed_paste(&self) -> bool {
        self.inner.modes.contains(Modes::BRACKETED_PASTE)
    }

    /// Strongest enabled mouse reporting mode (?1000/?1002/?1003).
    pub fn mouse_mode(&self) -> MouseMode {
        MouseMode::from_modes(self.inner.modes)
    }

    /// SGR mouse encoding (?1006).
    pub fn mouse_sgr(&self) -> bool {
        self.inner.modes.contains(Modes::MOUSE_SGR)
    }

    /// Alternate scroll (?1007): wheel sends arrow keys on the alternate
    /// screen. Defaults off, matching xterm.
    pub fn alternate_scroll(&self) -> bool {
        self.inner.modes.contains(Modes::ALT_SCROLL)
    }

    /// OSC 4 palette override for an index, if any.
    pub fn palette_override(&self, index: u8) -> Option<(u8, u8, u8)> {
        self.inner.palette[index as usize]
    }

    /// OSC 12 cursor color, if set (OSC 112 clears it).
    pub fn cursor_color(&self) -> Option<(u8, u8, u8)> {
        self.inner.cursor_color
    }

    /// Focus reporting (?1004): the program wants CSI I / CSI O on focus
    /// changes. The host calls [`Terminal::report_focus`] on window events.
    pub fn focus_reporting(&self) -> bool {
        self.inner.modes.contains(Modes::FOCUS_REPORT)
    }

    /// Emit a focus-in (CSI I) or focus-out (CSI O) report if the program
    /// enabled focus reporting; otherwise a no-op.
    pub fn report_focus(&mut self, focused: bool) {
        if self.focus_reporting() {
            self.inner
                .output
                .extend_from_slice(if focused { b"\x1b[I" } else { b"\x1b[O" });
        }
    }

    /// Synchronized output (?2026): while set, the host should hold off
    /// presenting frames so the program's update lands atomically.
    pub fn synchronized_output(&self) -> bool {
        self.inner.modes.contains(Modes::SYNC_OUTPUT)
    }

    /// Install the colors used to answer OSC 4/10/11/12 queries. Call this
    /// from the theme and refresh it on config reload.
    pub fn set_report_colors(&mut self, colors: report::ReportColors) {
        self.inner.report_colors = Some(Box::new(colors));
    }

    /// Take a pending OSC 52 clipboard write, if the program requested one.
    pub fn take_clipboard(&mut self) -> Option<report::Clipboard> {
        self.inner.clipboard.take()
    }

    /// Resolve an OSC 8 hyperlink id (from a [`Cell`]) to its target.
    pub fn hyperlink(&self, id: HyperlinkId) -> Option<&Hyperlink> {
        self.inner.hyperlinks.get(id)
    }

    /// The hyperlink URI a cell belongs to, if any.
    pub fn cell_hyperlink(&self, cell: &Cell) -> Option<&str> {
        cell.hyperlink
            .and_then(|id| self.inner.hyperlinks.get(id))
            .map(|link| link.uri.as_str())
    }

    /// Active kitty keyboard enhancement flags on the current screen (0 in
    /// legacy mode). Feed this into the input encoder.
    pub fn kitty_keyboard_flags(&self) -> u8 {
        self.inner.screen().kitty.current()
    }

    /// Search the whole buffer (scrollback + grid) for `needle`, returning
    /// matches in global-row order. `case_sensitive` false folds ASCII case.
    /// Matches do not span row breaks.
    pub fn search(&self, needle: &str, case_sensitive: bool) -> Vec<crate::search::Match> {
        let needle: Vec<char> = needle.chars().collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let grid = &self.inner.screen().grid;
        let sb = grid.scrollback();
        let mut out = Vec::new();
        let row_matches = |row: &Row, line: usize, out: &mut Vec<crate::search::Match>| {
            let mut chars = Vec::with_capacity(row.cells.len());
            let mut col_of = Vec::with_capacity(row.cells.len());
            for (c, cell) in row.cells.iter().enumerate() {
                if cell.is_wide_spacer() {
                    continue;
                }
                chars.push(cell.ch);
                col_of.push(c);
            }
            out.extend(crate::search::in_row(
                &needle,
                &chars,
                &col_of,
                line,
                !case_sensitive,
                |c| row.cells.get(c).is_some_and(|cell| cell.is_wide()),
            ));
        };
        for i in 0..sb.len() {
            if let Some(row) = sb.get(i) {
                row_matches(row, i, &mut out);
            }
        }
        for r in 0..grid.rows() {
            row_matches(grid.row(r), sb.len() + r, &mut out);
        }
        out
    }

    /// Text rows across scrollback + live grid in global-row order. Each
    /// tuple is `(line, text, prompt_marked)`, using the same line index
    /// space as [`Terminal::prompt_lines`].
    pub fn text_lines(&self) -> Vec<(usize, String, bool)> {
        let grid = &self.inner.screen().grid;
        let sb = grid.scrollback();
        let mut out = Vec::with_capacity(sb.len() + grid.rows());
        for i in 0..sb.len() {
            if let Some(row) = sb.get(i) {
                out.push((i, row.text(), row.prompt));
            }
        }
        for r in 0..grid.rows() {
            let row = grid.row(r);
            out.push((sb.len() + r, row.text(), row.prompt));
        }
        out
    }

    /// The URL under viewport row/col, if the text there is a detectable
    /// URL (used for click-to-open when there is no OSC 8 hyperlink).
    pub fn visible_url_at(&self, row: usize, col: usize) -> Option<String> {
        if row >= self.rows() {
            return None;
        }
        // Build the row's text and a char-index -> column map, skipping
        // wide spacers (a wide glyph occupies its head column plus one).
        let cells = &self.visible_row(row).cells;
        let mut chars: Vec<char> = Vec::with_capacity(cells.len());
        let mut col_of: Vec<usize> = Vec::with_capacity(cells.len());
        for (c, cell) in cells.iter().enumerate() {
            if cell.is_wide_spacer() {
                continue;
            }
            chars.push(cell.ch);
            col_of.push(c);
        }
        for (start, end) in crate::url::find(&chars) {
            let start_col = col_of[start];
            let last = col_of[end - 1];
            let end_col = last + if cells[last].is_wide() { 1 } else { 0 };
            if col >= start_col && col <= end_col {
                return Some(chars[start..end].iter().collect());
            }
        }
        None
    }

    /// Global indices of rows marked as shell prompts (OSC 133;A), sorted
    /// oldest first. Index space matches the viewport: `0..scrollback.len()`
    /// are history rows, `scrollback.len()..` are live-grid rows — so the
    /// top viewport row is `scrollback.len() - display_offset`. Used for
    /// jump-to-prompt.
    pub fn prompt_lines(&self) -> Vec<usize> {
        let grid = &self.inner.screen().grid;
        let sb = grid.scrollback();
        let mut lines = Vec::new();
        for i in 0..sb.len() {
            if sb.get(i).is_some_and(|r| r.prompt) {
                lines.push(i);
            }
        }
        for r in 0..grid.rows() {
            if grid.row(r).prompt {
                lines.push(sb.len() + r);
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_defaults() {
        let t = Terminal::new(80, 24, 100);
        assert_eq!(t.cols(), 80);
        assert_eq!(t.rows(), 24);
        assert!(!t.is_alt_screen());
        assert!(t.cursor_visible());
        assert_eq!(t.cursor_pos(), (0, 0));
        assert_eq!(t.title(), "");
        assert_eq!(t.cursor_style(), CursorStyle::BlinkingBlock);
    }

    #[test]
    fn feed_prints_text() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"hi");
        assert_eq!(t.row_text(0), "hi");
        assert_eq!(t.cursor_pos(), (0, 2));
    }

    #[test]
    fn visible_rows_with_offset() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc\r\nd");
        // "a" and "b" scrolled into history.
        assert_eq!(t.row_text(0), "c");
        t.set_display_offset(2);
        assert_eq!(t.row_text(0), "a");
        assert_eq!(t.row_text(1), "b");
        t.set_display_offset(99);
        assert_eq!(t.display_offset(), 2);
    }

    #[test]
    fn display_offset_stays_stable_as_output_arrives() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc\r\nd"); // scrollback: a, b
        t.set_display_offset(2);
        assert_eq!(t.row_text(0), "a");
        t.feed(b"\r\ne"); // "c" scrolls into history
        assert_eq!(t.display_offset(), 3);
        assert_eq!(t.row_text(0), "a"); // view did not shift
        assert_eq!(t.row_text(1), "b");
    }

    #[test]
    fn display_offset_clamps_when_ring_evicts() {
        let mut t = Terminal::new(4, 2, 2);
        t.feed(b"a\r\nb\r\nc\r\nd"); // ring full: a, b
        t.set_display_offset(2);
        t.feed(b"\r\ne"); // pushes "c", evicts "a"
        assert_eq!(t.display_offset(), 2); // clamped to ring length
        assert_eq!(t.row_text(0), "b");
    }

    #[test]
    fn display_offset_untouched_at_bottom() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc");
        assert_eq!(t.display_offset(), 0);
        t.feed(b"\r\nd"); // more history, still at the live bottom
        assert_eq!(t.display_offset(), 0);
    }

    #[test]
    fn scroll_display_deltas_clamp() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc\r\nd"); // scrollback len 2
        t.scroll_display(99);
        assert_eq!(t.display_offset(), 2);
        t.scroll_display(-1);
        assert_eq!(t.display_offset(), 1);
        t.scroll_display(-99);
        assert_eq!(t.display_offset(), 0);
    }

    #[test]
    fn entering_alt_resets_display_offset() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc\r\nd");
        t.set_display_offset(2);
        t.feed(b"\x1b[?1049h");
        assert_eq!(t.display_offset(), 0);
        // No scrollback on alt: scrolling is a no-op.
        t.scroll_display(5);
        assert_eq!(t.display_offset(), 0);
        // vt does not restore the offset on exit; the app decides.
        t.feed(b"\x1b[?1049l");
        assert_eq!(t.display_offset(), 0);
    }

    #[test]
    fn mouse_mode_tracks_decset() {
        use crate::mode::MouseMode;
        let mut t = Terminal::new(10, 3, 0);
        assert_eq!(t.mouse_mode(), MouseMode::None);
        assert!(!t.mouse_sgr());
        t.feed(b"\x1b[?1000h");
        assert_eq!(t.mouse_mode(), MouseMode::Click);
        t.feed(b"\x1b[?1002h");
        assert_eq!(t.mouse_mode(), MouseMode::Drag);
        t.feed(b"\x1b[?1003h");
        assert_eq!(t.mouse_mode(), MouseMode::Motion);
        t.feed(b"\x1b[?1006h");
        assert!(t.mouse_sgr());
        t.feed(b"\x1b[?1003l");
        assert_eq!(t.mouse_mode(), MouseMode::Drag);
        t.feed(b"\x1b[?1002l\x1b[?1000l\x1b[?1006l");
        assert_eq!(t.mouse_mode(), MouseMode::None);
        assert!(!t.mouse_sgr());
    }

    #[test]
    fn alternate_scroll_defaults_off_and_tracks_1007() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.alternate_scroll()); // xterm default
        t.feed(b"\x1b[?1007h");
        assert!(t.alternate_scroll());
        t.feed(b"\x1b[?1007l");
        assert!(!t.alternate_scroll());
    }

    #[test]
    fn resize_clamps_display_offset() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc");
        t.set_display_offset(1);
        t.resize(6, 4);
        assert!(t.display_offset() <= t.grid().scrollback().len());
        assert_eq!(t.cols(), 6);
        assert_eq!(t.rows(), 4);
    }

    #[test]
    fn fresh_terminal_is_fully_damaged() {
        let mut t = Terminal::new(10, 3, 0);
        assert_eq!(t.take_damage(), Damage::Full);
        assert_eq!(t.take_damage(), Damage::Rows(vec![]));
    }

    #[test]
    fn printing_marks_row_dirty_and_take_clears() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.feed(b"hi");
        assert_eq!(t.take_damage(), Damage::Rows(vec![0]));
        assert_eq!(t.take_damage(), Damage::Rows(vec![]));
        t.feed(b"\x1b[3;1Hx");
        assert_eq!(t.take_damage(), Damage::Rows(vec![2]));
    }

    #[test]
    fn scroll_escalates_to_full_damage() {
        let mut t = Terminal::new(4, 2, 10);
        t.take_damage();
        t.feed(b"a\r\nb\r\nc"); // last linefeed scrolls
        assert_eq!(t.take_damage(), Damage::Full);
    }

    #[test]
    fn resize_escalates_to_full_damage() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.resize(8, 4);
        assert_eq!(t.take_damage(), Damage::Full);
    }

    #[test]
    fn alt_switch_escalates_to_full_damage() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.feed(b"\x1b[?1049h");
        assert_eq!(t.take_damage(), Damage::Full);
        t.feed(b"\x1b[?1049l");
        assert_eq!(t.take_damage(), Damage::Full);
        // Leaving alt while already on primary changes nothing.
        t.feed(b"\x1b[?1049l");
        assert_eq!(t.take_damage(), Damage::Rows(vec![]));
    }

    #[test]
    fn ris_and_ed_escalate_to_full_damage() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.feed(b"\x1bc");
        assert_eq!(t.take_damage(), Damage::Full);
        t.feed(b"\x1b[2J");
        assert_eq!(t.take_damage(), Damage::Full);
        t.feed(b"\x1b[3J");
        assert_eq!(t.take_damage(), Damage::Full);
    }

    #[test]
    fn palette_osc_escalates_to_full_damage() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.feed(b"\x1b]4;1;rgb:ff/00/00\x07");
        assert_eq!(t.take_damage(), Damage::Full);
        t.feed(b"\x1b]104\x07");
        assert_eq!(t.take_damage(), Damage::Full);
    }

    #[test]
    fn display_offset_change_escalates_to_full_damage() {
        let mut t = Terminal::new(4, 2, 10);
        t.feed(b"a\r\nb\r\nc");
        t.take_damage();
        t.set_display_offset(1);
        assert_eq!(t.take_damage(), Damage::Full);
        // Setting the same offset again is not damage.
        t.set_display_offset(1);
        assert_eq!(t.take_damage(), Damage::Rows(vec![]));
    }

    #[test]
    fn full_damage_clears_stale_row_damage() {
        let mut t = Terminal::new(10, 3, 0);
        t.take_damage();
        t.feed(b"hi");
        t.resize(8, 4);
        assert_eq!(t.take_damage(), Damage::Full);
        assert_eq!(t.take_damage(), Damage::Rows(vec![]));
    }

    #[test]
    fn bell_take_and_clear() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.take_bell());
        t.feed(b"\x07");
        assert!(t.take_bell());
        assert!(!t.take_bell());
    }

    #[test]
    fn title_change_signal() {
        let mut t = Terminal::new(10, 3, 0);
        assert_eq!(t.take_title_changed(), None);
        t.feed(b"\x1b]2;hello\x07");
        assert_eq!(t.take_title_changed(), Some("hello".to_string()));
        assert_eq!(t.take_title_changed(), None);
        assert_eq!(t.title(), "hello");
        t.feed(b"\x1b]0;again\x07");
        assert_eq!(t.take_title_changed(), Some("again".to_string()));
    }

    #[test]
    fn cursor_keys_app_tracks_decckm() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.cursor_keys_app());
        t.feed(b"\x1b[?1h");
        assert!(t.cursor_keys_app());
        t.feed(b"\x1b[?1l");
        assert!(!t.cursor_keys_app());
        // RIS clears it.
        t.feed(b"\x1b[?1h\x1bc");
        assert!(!t.cursor_keys_app());
    }

    #[test]
    fn keypad_app_tracks_deckpam_deckpnm_and_ris() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.keypad_app());
        t.feed(b"\x1b=");
        assert!(t.keypad_app());
        t.feed(b"\x1b>");
        assert!(!t.keypad_app());
        t.feed(b"\x1b=\x1bc");
        assert!(!t.keypad_app());
    }

    #[test]
    fn bracketed_paste_accessor() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.bracketed_paste());
        t.feed(b"\x1b[?2004h");
        assert!(t.bracketed_paste());
        t.feed(b"\x1b[?2004l");
        assert!(!t.bracketed_paste());
        t.feed(b"\x1b[?2004h\x1bc");
        assert!(!t.bracketed_paste());
    }

    #[test]
    fn focus_reporting_emits_only_when_enabled() {
        let mut t = Terminal::new(10, 3, 0);
        // Off by default: report_focus is a no-op.
        assert!(!t.focus_reporting());
        t.report_focus(true);
        assert!(t.take_output().is_empty());
        // Enable ?1004 and focus in/out emit CSI I / CSI O.
        t.feed(b"\x1b[?1004h");
        assert!(t.focus_reporting());
        t.report_focus(true);
        assert_eq!(t.take_output(), b"\x1b[I");
        t.report_focus(false);
        assert_eq!(t.take_output(), b"\x1b[O");
        t.feed(b"\x1b[?1004l");
        t.report_focus(true);
        assert!(t.take_output().is_empty());
    }

    #[test]
    fn synchronized_output_tracks_2026() {
        let mut t = Terminal::new(10, 3, 0);
        assert!(!t.synchronized_output());
        t.feed(b"\x1b[?2026h");
        assert!(t.synchronized_output());
        t.feed(b"\x1b[?2026l");
        assert!(!t.synchronized_output());
    }

    #[test]
    fn search_finds_matches_across_scrollback() {
        let mut t = Terminal::new(10, 2, 10);
        t.feed(b"foo bar\r\nbaz foo\r\nqux"); // "foo" on lines 0 and 1
        let hits = t.search("foo", false);
        assert_eq!(hits.len(), 2);
        // First match is the oldest (lowest global line), col 0.
        assert_eq!((hits[0].start_col, hits[0].end_col), (0, 2));
        // Case-insensitive by default; case-sensitive can differ.
        t.feed(b"\r\nFOO");
        assert_eq!(t.search("foo", false).len(), 3);
        assert_eq!(t.search("foo", true).len(), 2);
    }

    #[test]
    fn da2_reports_secondary_attributes() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"\x1b[>c");
        assert_eq!(t.take_output(), b"\x1b[>0;276;0c");
    }

    #[test]
    fn title_stack_pop_signals_change() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"\x1b]2;first\x07\x1b[22;0t\x1b]2;second\x07");
        t.take_title_changed();
        t.feed(b"\x1b[23;0t");
        assert_eq!(t.take_title_changed(), Some("first".to_string()));
    }
}
