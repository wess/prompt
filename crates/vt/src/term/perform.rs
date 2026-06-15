//! `vte::Perform` for the terminal state: print, C0, ESC, and delegation
//! to the CSI/OSC handlers.

use crate::charset;
use crate::mode::Modes;

use super::{csi, dcs, osc, Inner};

impl vte::Perform for Inner {
    fn print(&mut self, c: char) {
        let mapped = self.charsets.map(c);
        self.write_char(mapped);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.bell = true,
            0x08 => self.cursor_left(1),
            0x09 => self.tab_forward(1),
            0x0a | 0x0b | 0x0c => self.linefeed(),
            0x0d => self.carriage_return(),
            0x0e => self.charsets.shifted = true,
            0x0f => self.charsets.shifted = false,
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates, byte) {
            ([], b'7') => self.save_cursor(),
            ([], b'8') => self.restore_cursor(),
            ([], b'D') => self.linefeed(),
            ([], b'E') => {
                self.carriage_return();
                self.linefeed();
            }
            ([], b'H') => {
                let col = self.screen().cursor.col;
                self.screen_mut().set_tab(col);
            }
            ([], b'M') => self.reverse_index(),
            ([], b'c') => self.full_reset(),
            ([], b'=') => self.modes.insert(Modes::APP_KEYPAD),
            ([], b'>') => self.modes.remove(Modes::APP_KEYPAD),
            ([b'('], f) => self.charsets.g0 = charset::from_final(f),
            ([b')'], f) => self.charsets.g1 = charset::from_final(f),
            ([b'#'], b'8') => self.screen_alignment_test(),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        csi::dispatch(self, params, intermediates, action);
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        osc::dispatch(self, params, bell_terminated);
    }

    fn hook(&mut self, _params: &vte::Params, intermediates: &[u8], _ignore: bool, action: char) {
        dcs::hook(self, intermediates, action);
    }

    fn put(&mut self, byte: u8) {
        dcs::put(self, byte);
    }

    fn unhook(&mut self) {
        dcs::unhook(self);
    }
}

#[cfg(test)]
mod tests {
    use crate::charset::Charset;
    use crate::term::Terminal;

    #[test]
    fn bel_sets_flag() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"\x07");
        assert!(t.take_bell());
        assert!(!t.take_bell());
    }

    #[test]
    fn backspace_moves_left() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"ab\x08c");
        assert_eq!(t.row_text(0), "ac");
    }

    #[test]
    fn ht_moves_to_tab_stop() {
        let mut t = Terminal::new(20, 3, 0);
        t.feed(b"\tx");
        assert_eq!(t.cell(0, 8).ch, 'x');
    }

    #[test]
    fn nel_is_cr_plus_lf() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"ab\x1bEc");
        assert_eq!(t.row_text(1), "c");
        assert_eq!(t.cursor_pos(), (1, 1));
    }

    #[test]
    fn hts_sets_tab_stop() {
        let mut t = Terminal::new(20, 3, 0);
        t.feed(b"\x1b[5G\x1bH\r\tx");
        assert_eq!(t.cell(0, 4).ch, 'x');
    }

    #[test]
    fn keypad_modes() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"\x1b=");
        assert!(t.modes().contains(crate::Modes::APP_KEYPAD));
        t.feed(b"\x1b>");
        assert!(!t.modes().contains(crate::Modes::APP_KEYPAD));
    }

    #[test]
    fn charset_designation_and_shift() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"\x1b(0");
        assert_eq!(t.inner.charsets.g0, Charset::DecSpecial);
        t.feed(b"q");
        assert_eq!(t.cell(0, 0).ch, '─');
        t.feed(b"\x1b(B");
        t.feed(b"q");
        assert_eq!(t.cell(0, 1).ch, 'q');
        // SO selects G1.
        t.feed(b"\x1b)0\x0eq\x0fq");
        assert_eq!(t.cell(0, 2).ch, '─');
        assert_eq!(t.cell(0, 3).ch, 'q');
    }

    #[test]
    fn ris_resets() {
        let mut t = Terminal::new(10, 3, 0);
        t.feed(b"hello\x1b[?25l\x1bc");
        assert_eq!(t.row_text(0), "");
        assert!(t.cursor_visible());
    }
}
