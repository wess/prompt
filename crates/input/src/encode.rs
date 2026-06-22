//! `encode_key`: one keystroke -> the bytes a legacy xterm would send.

use crate::csi;
use crate::kitty;
use crate::{Mods, TermState};

/// Encode a keystroke. `key` is the gpui keystroke name; `text` is the
/// platform-resolved typed text for printable keys. Returns `None` when the
/// key produces no pty bytes (cmd chords, unknown non-printables).
pub fn encode_key(key: &str, text: Option<&str>, mods: Mods, state: TermState) -> Option<Vec<u8>> {
    if mods.cmd {
        return None;
    }
    // Kitty keyboard protocol intercepts the keys it disambiguates; others
    // fall through to legacy encoding below.
    if state.kitty_flags != 0 {
        if let Some(bytes) = kitty::encode(key, mods, state.kitty_flags) {
            return Some(bytes);
        }
    }
    if let Some(bytes) = special(key, mods, state) {
        return Some(bytes);
    }
    if mods.ctrl {
        if let Some(byte) = ctrl_byte(key) {
            return Some(alt_prefixed(mods, vec![byte]));
        }
    }
    printable(key, text, mods)
}

/// Named (non-printable) keys; `None` means "not a special key".
fn special(key: &str, mods: Mods, state: TermState) -> Option<Vec<u8>> {
    let bytes = match key {
        "enter" => alt_prefixed(mods, vec![b'\r']),
        "tab" => {
            if mods.shift {
                alt_prefixed(mods, vec![csi::ESC, b'[', b'Z'])
            } else {
                alt_prefixed(mods, vec![b'\t'])
            }
        }
        "escape" => alt_prefixed(mods, vec![csi::ESC]),
        "backspace" => {
            let byte = if mods.ctrl { 0x08 } else { 0x7f };
            alt_prefixed(mods, vec![byte])
        }
        "up" => cursor_key(b'A', mods, state),
        "down" => cursor_key(b'B', mods, state),
        "right" => cursor_key(b'C', mods, state),
        "left" => cursor_key(b'D', mods, state),
        "home" => cursor_key(b'H', mods, state),
        "end" => cursor_key(b'F', mods, state),
        "insert" => csi::tilde(2, mods),
        "delete" => csi::tilde(3, mods),
        "pageup" => csi::tilde(5, mods),
        "pagedown" => csi::tilde(6, mods),
        "f1" => fkey_ss3(b'P', mods),
        "f2" => fkey_ss3(b'Q', mods),
        "f3" => fkey_ss3(b'R', mods),
        "f4" => fkey_ss3(b'S', mods),
        "f5" => csi::tilde(15, mods),
        "f6" => csi::tilde(17, mods),
        "f7" => csi::tilde(18, mods),
        "f8" => csi::tilde(19, mods),
        "f9" => csi::tilde(20, mods),
        "f10" => csi::tilde(21, mods),
        "f11" => csi::tilde(23, mods),
        "f12" => csi::tilde(24, mods),
        _ => return None,
    };
    Some(bytes)
}

/// Arrows/home/end: CSI normally, SS3 in app mode, modified CSI form when
/// any modifier is held (modifiers force CSI even in app mode).
fn cursor_key(final_byte: u8, mods: Mods, state: TermState) -> Vec<u8> {
    if csi::is_modified(mods) {
        csi::cursor_modified(final_byte, mods)
    } else {
        csi::cursor(final_byte, state.cursor_keys_app)
    }
}

/// f1-f4: SS3 unmodified, `CSI 1;{m}{final}` modified.
fn fkey_ss3(final_byte: u8, mods: Mods) -> Vec<u8> {
    if csi::is_modified(mods) {
        csi::cursor_modified(final_byte, mods)
    } else {
        csi::ss3(final_byte)
    }
}

/// The legacy ctrl-key table: letters -> 0x01..0x1a plus the punctuation
/// control bytes.
fn ctrl_byte(key: &str) -> Option<u8> {
    if key == "space" {
        return Some(0x00);
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1),
        '@' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' | '-' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        _ => None,
    }
}

/// Printable path: emit `text` as UTF-8; alt (without ctrl) ESC-prefixes a
/// single ASCII char and passes non-ASCII text through unchanged.
fn printable(key: &str, text: Option<&str>, mods: Mods) -> Option<Vec<u8>> {
    let text = match text {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => fallback_text(key)?,
    };
    if mods.alt && !mods.ctrl {
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            if c.is_ascii() {
                return Some(vec![csi::ESC, c as u8]);
            }
        }
    }
    Some(text.into_bytes())
}

/// Derive text from the key name when the platform supplied none.
fn fallback_text(key: &str) -> Option<String> {
    if key == "space" {
        return Some(" ".to_string());
    }
    let mut chars = key.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(c.to_string())
}

fn alt_prefixed(mods: Mods, mut bytes: Vec<u8>) -> Vec<u8> {
    if mods.alt {
        bytes.insert(0, csi::ESC);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Mods = Mods {
        shift: false,
        alt: false,
        ctrl: false,
        cmd: false,
    };
    const SHIFT: Mods = Mods {
        shift: true,
        ..NONE
    };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const CMD: Mods = Mods { cmd: true, ..NONE };
    const CTRL_ALT: Mods = Mods {
        ctrl: true,
        alt: true,
        ..NONE
    };
    const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        ..NONE
    };
    const ALT_SHIFT: Mods = Mods {
        alt: true,
        shift: true,
        ..NONE
    };
    const ALL: Mods = Mods {
        shift: true,
        alt: true,
        ctrl: true,
        cmd: false,
    };

    const NORMAL: TermState = TermState {
        cursor_keys_app: false,
        keypad_app: false,
        bracketed_paste: false,
        kitty_flags: 0,
    };
    const APP: TermState = TermState {
        cursor_keys_app: true,
        keypad_app: true,
        bracketed_paste: false,
        kitty_flags: 0,
    };

    fn enc(key: &str, text: Option<&str>, mods: Mods, state: TermState) -> Option<Vec<u8>> {
        encode_key(key, text, mods, state)
    }

    #[test]
    fn cmd_chords_return_none() {
        for key in ["a", "enter", "up", "f5", "delete", "space"] {
            assert_eq!(enc(key, Some("a"), CMD, NORMAL), None, "{key}");
            let with_ctrl = Mods { ctrl: true, ..CMD };
            assert_eq!(enc(key, Some("a"), with_ctrl, NORMAL), None, "{key}");
        }
    }

    #[test]
    fn printable_emits_text() {
        let cases: &[(&str, Option<&str>, Mods, &[u8])] = &[
            ("a", Some("a"), NONE, b"a"),
            ("a", Some("A"), SHIFT, b"A"), // shift pre-resolved by platform
            ("1", Some("1"), NONE, b"1"),
            ("1", Some("!"), SHIFT, b"!"),
            ("/", Some("/"), NONE, b"/"),
            ("space", Some(" "), NONE, b" "),
            ("e", Some("\u{e9}"), NONE, "é".as_bytes()), // dead-key result
            // Fallbacks when the platform supplies no text.
            ("/", None, NONE, b"/"),
            ("space", None, NONE, b" "),
            ("z", None, NONE, b"z"),
        ];
        for (key, text, mods, want) in cases {
            assert_eq!(
                enc(key, *text, *mods, NORMAL).as_deref(),
                Some(*want),
                "{key} {text:?}"
            );
        }
    }

    #[test]
    fn alt_printable_prefixes_esc_for_single_ascii() {
        let cases: &[(&str, Option<&str>, Mods, &[u8])] = &[
            ("a", Some("a"), ALT, b"\x1ba"),
            ("x", Some("X"), ALT_SHIFT, b"\x1bX"),
            ("space", Some(" "), ALT, b"\x1b "),
            ("1", Some("1"), ALT, b"\x1b1"),
            // Non-ASCII alt text passes through unchanged (macOS option).
            ("e", Some("\u{e9}"), ALT, "é".as_bytes()),
            ("o", Some("\u{f8}"), ALT, "ø".as_bytes()),
            // Multi-char text passes through unchanged.
            ("a", Some("ab"), ALT, b"ab"),
        ];
        for (key, text, mods, want) in cases {
            assert_eq!(
                enc(key, *text, *mods, NORMAL).as_deref(),
                Some(*want),
                "{key} {text:?}"
            );
        }
    }

    #[test]
    fn ctrl_letter_full_table() {
        for (i, key) in ('a'..='z').enumerate() {
            let want = vec![i as u8 + 1];
            assert_eq!(
                enc(&key.to_string(), Some(&key.to_string()), CTRL, NORMAL),
                Some(want.clone()),
                "ctrl+{key}"
            );
            // shift+ctrl+letter encodes the same byte.
            assert_eq!(
                enc(
                    &key.to_string(),
                    Some(&key.to_ascii_uppercase().to_string()),
                    CTRL_SHIFT,
                    NORMAL
                ),
                Some(want),
                "ctrl+shift+{key}"
            );
        }
        // The two overlaps with C0 names are intentional.
        assert_eq!(enc("i", Some("i"), CTRL, NORMAL).unwrap(), b"\t");
        assert_eq!(enc("m", Some("m"), CTRL, NORMAL).unwrap(), b"\r");
    }

    #[test]
    fn ctrl_punctuation_table() {
        let cases: &[(&str, u8)] = &[
            ("space", 0x00),
            ("@", 0x00),
            ("[", 0x1b),
            ("\\", 0x1c),
            ("]", 0x1d),
            ("^", 0x1e),
            ("_", 0x1f),
            ("-", 0x1f),
            ("?", 0x7f),
            ("8", 0x7f),
        ];
        for (key, byte) in cases {
            assert_eq!(
                enc(key, Some(key), CTRL, NORMAL),
                Some(vec![*byte]),
                "ctrl+{key}"
            );
        }
    }

    #[test]
    fn ctrl_alt_prefixes_esc() {
        assert_eq!(enc("a", Some("a"), CTRL_ALT, NORMAL).unwrap(), b"\x1b\x01");
        assert_eq!(
            enc("space", Some(" "), CTRL_ALT, NORMAL).unwrap(),
            b"\x1b\x00"
        );
    }

    #[test]
    fn ctrl_unmapped_falls_back_to_text() {
        // xterm sends the plain character for ctrl+digit outside the table.
        assert_eq!(enc("1", Some("1"), CTRL, NORMAL).unwrap(), b"1");
    }

    #[test]
    fn enter_tab_escape_backspace() {
        let cases: &[(&str, Mods, &[u8])] = &[
            ("enter", NONE, b"\r"),
            ("enter", ALT, b"\x1b\r"),
            ("tab", NONE, b"\t"),
            ("tab", SHIFT, b"\x1b[Z"),
            ("tab", ALT, b"\x1b\t"),
            ("tab", ALT_SHIFT, b"\x1b\x1b[Z"),
            ("escape", NONE, b"\x1b"),
            ("escape", ALT, b"\x1b\x1b"),
            ("backspace", NONE, b"\x7f"),
            ("backspace", CTRL, b"\x08"),
            ("backspace", ALT, b"\x1b\x7f"),
            ("backspace", CTRL_ALT, b"\x1b\x08"),
        ];
        for (key, mods, want) in cases {
            assert_eq!(
                enc(key, None, *mods, NORMAL).as_deref(),
                Some(*want),
                "{key} {mods:?}"
            );
        }
    }

    #[test]
    fn arrows_normal_and_app_mode() {
        let cases: &[(&str, u8)] = &[
            ("up", b'A'),
            ("down", b'B'),
            ("right", b'C'),
            ("left", b'D'),
        ];
        for (key, fin) in cases {
            assert_eq!(
                enc(key, None, NONE, NORMAL).unwrap(),
                vec![0x1b, b'[', *fin],
                "{key} normal"
            );
            assert_eq!(
                enc(key, None, NONE, APP).unwrap(),
                vec![0x1b, b'O', *fin],
                "{key} app"
            );
        }
    }

    #[test]
    fn modified_arrows_all_combos_force_csi() {
        let combos: &[(Mods, &str)] = &[
            (SHIFT, "2"),
            (ALT, "3"),
            (ALT_SHIFT, "4"),
            (CTRL, "5"),
            (CTRL_SHIFT, "6"),
            (CTRL_ALT, "7"),
            (ALL, "8"),
        ];
        for (mods, m) in combos {
            let want = format!("\x1b[1;{m}A").into_bytes();
            // Same bytes in normal and app mode: modifiers force CSI.
            assert_eq!(enc("up", None, *mods, NORMAL).unwrap(), want, "{mods:?}");
            assert_eq!(enc("up", None, *mods, APP).unwrap(), want, "{mods:?} app");
        }
        assert_eq!(enc("down", None, CTRL, APP).unwrap(), b"\x1b[1;5B");
        assert_eq!(enc("right", None, SHIFT, APP).unwrap(), b"\x1b[1;2C");
        assert_eq!(enc("left", None, ALT, APP).unwrap(), b"\x1b[1;3D");
    }

    #[test]
    fn home_end_forms() {
        assert_eq!(enc("home", None, NONE, NORMAL).unwrap(), b"\x1b[H");
        assert_eq!(enc("end", None, NONE, NORMAL).unwrap(), b"\x1b[F");
        assert_eq!(enc("home", None, NONE, APP).unwrap(), b"\x1bOH");
        assert_eq!(enc("end", None, NONE, APP).unwrap(), b"\x1bOF");
        assert_eq!(enc("home", None, CTRL, APP).unwrap(), b"\x1b[1;5H");
        assert_eq!(enc("end", None, ALL, NORMAL).unwrap(), b"\x1b[1;8F");
    }

    #[test]
    fn tilde_keys() {
        let cases: &[(&str, u8)] = &[("insert", 2), ("delete", 3), ("pageup", 5), ("pagedown", 6)];
        for (key, n) in cases {
            assert_eq!(
                enc(key, None, NONE, NORMAL).unwrap(),
                format!("\x1b[{n}~").into_bytes(),
                "{key}"
            );
            // App-mode state never changes tilde keys.
            assert_eq!(
                enc(key, None, NONE, APP).unwrap(),
                format!("\x1b[{n}~").into_bytes(),
                "{key} app"
            );
        }
        let combos: &[(Mods, &str)] = &[
            (SHIFT, "2"),
            (ALT, "3"),
            (ALT_SHIFT, "4"),
            (CTRL, "5"),
            (CTRL_SHIFT, "6"),
            (CTRL_ALT, "7"),
            (ALL, "8"),
        ];
        for (mods, m) in combos {
            assert_eq!(
                enc("delete", None, *mods, NORMAL).unwrap(),
                format!("\x1b[3;{m}~").into_bytes(),
                "delete {mods:?}"
            );
        }
        assert_eq!(enc("pageup", None, SHIFT, NORMAL).unwrap(), b"\x1b[5;2~");
        assert_eq!(enc("pagedown", None, CTRL, NORMAL).unwrap(), b"\x1b[6;5~");
        assert_eq!(enc("insert", None, ALT, NORMAL).unwrap(), b"\x1b[2;3~");
    }

    #[test]
    fn fkeys_f1_to_f4() {
        let cases: &[(&str, u8)] = &[("f1", b'P'), ("f2", b'Q'), ("f3", b'R'), ("f4", b'S')];
        for (key, fin) in cases {
            assert_eq!(
                enc(key, None, NONE, NORMAL).unwrap(),
                vec![0x1b, b'O', *fin],
                "{key}"
            );
            assert_eq!(
                enc(key, None, CTRL, NORMAL).unwrap(),
                format!("\x1b[1;5{}", *fin as char).into_bytes(),
                "ctrl+{key}"
            );
            assert_eq!(
                enc(key, None, SHIFT, NORMAL).unwrap(),
                format!("\x1b[1;2{}", *fin as char).into_bytes(),
                "shift+{key}"
            );
        }
    }

    #[test]
    fn fkeys_f5_to_f12() {
        let cases: &[(&str, u8)] = &[
            ("f5", 15),
            ("f6", 17),
            ("f7", 18),
            ("f8", 19),
            ("f9", 20),
            ("f10", 21),
            ("f11", 23),
            ("f12", 24),
        ];
        for (key, n) in cases {
            assert_eq!(
                enc(key, None, NONE, NORMAL).unwrap(),
                format!("\x1b[{n}~").into_bytes(),
                "{key}"
            );
            assert_eq!(
                enc(key, None, CTRL_SHIFT, NORMAL).unwrap(),
                format!("\x1b[{n};6~").into_bytes(),
                "ctrl+shift+{key}"
            );
        }
    }

    #[test]
    fn unknown_key_without_text_is_none() {
        assert_eq!(enc("f13", None, NONE, NORMAL), None);
        assert_eq!(enc("menu", None, NONE, NORMAL), None);
    }
}
