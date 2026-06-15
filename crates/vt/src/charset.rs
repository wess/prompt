//! G0/G1 character set designation and DEC special graphics mapping.

/// A designable character set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Ascii,
    /// DEC Special Graphics (line drawing), designated with final byte `0`.
    DecSpecial,
}

/// G0/G1 slots plus which one is active (SI selects G0, SO selects G1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Charsets {
    pub g0: Charset,
    pub g1: Charset,
    /// `true` after SO (G1 active), `false` after SI (G0 active).
    pub shifted: bool,
}

impl Charsets {
    pub fn active(&self) -> Charset {
        if self.shifted {
            self.g1
        } else {
            self.g0
        }
    }

    /// Map a printable character through the active charset.
    pub fn map(&self, c: char) -> char {
        match self.active() {
            Charset::Ascii => c,
            Charset::DecSpecial => dec_special(c),
        }
    }
}

/// Charset for an SCS final byte (`ESC ( F` / `ESC ) F`).
pub fn from_final(byte: u8) -> Charset {
    match byte {
        b'0' => Charset::DecSpecial,
        _ => Charset::Ascii,
    }
}

/// DEC Special Graphics: maps `_` and `` ` `` through `~` to line-drawing
/// and symbol glyphs; everything else passes through.
pub fn dec_special(c: char) -> char {
    match c {
        '_' => ' ',
        '`' => '◆',
        'a' => '▒',
        'b' => '␉',
        'c' => '␌',
        'd' => '␍',
        'e' => '␊',
        'f' => '°',
        'g' => '±',
        'h' => '␤',
        'i' => '␋',
        'j' => '┘',
        'k' => '┐',
        'l' => '┌',
        'm' => '└',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'q' => '─',
        'r' => '⎼',
        's' => '⎽',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'x' => '│',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        '~' => '·',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        let cs = Charsets::default();
        assert_eq!(cs.map('q'), 'q');
        assert_eq!(cs.map('A'), 'A');
    }

    #[test]
    fn dec_special_maps_line_drawing() {
        let cs = Charsets {
            g0: Charset::DecSpecial,
            ..Default::default()
        };
        assert_eq!(cs.map('q'), '─');
        assert_eq!(cs.map('x'), '│');
        assert_eq!(cs.map('l'), '┌');
        assert_eq!(cs.map('j'), '┘');
        assert_eq!(cs.map('A'), 'A');
    }

    #[test]
    fn shift_out_selects_g1() {
        let mut cs = Charsets {
            g1: Charset::DecSpecial,
            ..Default::default()
        };
        assert_eq!(cs.map('q'), 'q');
        cs.shifted = true;
        assert_eq!(cs.map('q'), '─');
        cs.shifted = false;
        assert_eq!(cs.map('q'), 'q');
    }

    #[test]
    fn final_byte_designation() {
        assert_eq!(from_final(b'0'), Charset::DecSpecial);
        assert_eq!(from_final(b'B'), Charset::Ascii);
        assert_eq!(from_final(b'A'), Charset::Ascii);
    }
}
