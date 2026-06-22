//! Kitty keyboard protocol key encoding.
//!
//! This intercepts only the keys the protocol *disambiguates* — Escape and
//! ctrl/alt-modified or all-keys-mode text keys — emitting the `CSI
//! codepoint ; modifiers u` form. Everything else (plain text, arrows,
//! function keys, …) returns `None` so the legacy encoder handles it
//! unchanged; in kitty mode those still produce valid (unambiguous) escape
//! codes.
//!
//! Limitations: only press events are emitted (the host does not deliver
//! distinct release events), so the event-type suffix is never added and
//! the `report_event_types`/`report_associated_text`/`report_alternate_keys`
//! flags are tracked-but-not-encoded. `report_all_keys_as_escape_codes`
//! upgrades text keys to `CSI u`; navigation/function keys keep their legacy
//! (still-escape-code) spellings.

use crate::csi;
use crate::kitty_flags;
use crate::Mods;

/// Encode a keystroke under active kitty flags. Returns `None` for keys
/// this layer does not special-case, leaving them to the legacy encoder.
pub(crate) fn encode(key: &str, mods: Mods, flags: u8) -> Option<Vec<u8>> {
    let codepoint = csi_u_codepoint(key)?;
    let all_keys = flags & kitty_flags::REPORT_ALL_KEYS_AS_ESCAPE_CODES != 0;
    // Escape is always disambiguated; other text/special keys only when a
    // ctrl/alt modifier (which would otherwise collapse to a control byte
    // or ESC-prefix) is held, or when all keys are requested as escapes.
    let force = all_keys || key == "escape" || mods.ctrl || mods.alt;
    force.then(|| csi_u(codepoint, mods))
}

/// The CSI-u key code for a disambiguated key, or `None` for keys that keep
/// their legacy navigation/function encodings.
fn csi_u_codepoint(key: &str) -> Option<u32> {
    Some(match key {
        "escape" => 27,
        "enter" => 13,
        "tab" => 9,
        "backspace" => 127,
        "space" => 32,
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => c as u32,
                _ => return None,
            }
        }
    })
}

/// `CSI codepoint u`, or `CSI codepoint ; modifiers u` when modified.
fn csi_u(codepoint: u32, mods: Mods) -> Vec<u8> {
    let m = csi::modifier_param(mods);
    if m == 1 {
        format!("\x1b[{codepoint}u").into_bytes()
    } else {
        format!("\x1b[{codepoint};{m}u").into_bytes()
    }
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
    const CTRL: Mods = Mods { ctrl: true, ..NONE };
    const ALT: Mods = Mods { alt: true, ..NONE };
    const CTRL_SHIFT: Mods = Mods {
        ctrl: true,
        shift: true,
        ..NONE
    };

    const DIS: u8 = kitty_flags::DISAMBIGUATE;
    const ALL: u8 = kitty_flags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;

    #[test]
    fn escape_always_disambiguated() {
        assert_eq!(encode("escape", NONE, DIS).unwrap(), b"\x1b[27u");
        assert_eq!(encode("escape", SHIFT, DIS).unwrap(), b"\x1b[27;2u");
    }

    #[test]
    fn ctrl_letter_becomes_csi_u() {
        // ctrl+a: codepoint 97, modifiers 1+ctrl(4) = 5.
        assert_eq!(encode("a", CTRL, DIS).unwrap(), b"\x1b[97;5u");
        // ctrl+shift+a: 1+shift(1)+ctrl(4) = 6.
        assert_eq!(encode("a", CTRL_SHIFT, DIS).unwrap(), b"\x1b[97;6u");
    }

    #[test]
    fn alt_letter_becomes_csi_u() {
        // alt+a: 1+alt(2) = 3.
        assert_eq!(encode("a", ALT, DIS).unwrap(), b"\x1b[97;3u");
    }

    #[test]
    fn plain_and_shift_text_defer_to_legacy_unless_all_keys() {
        // Disambiguate only: plain/shifted printable returns None (legacy).
        assert_eq!(encode("a", NONE, DIS), None);
        assert_eq!(encode("a", SHIFT, DIS), None);
        assert_eq!(encode("1", NONE, DIS), None);
        // All-keys mode upgrades them to CSI u.
        assert_eq!(encode("a", NONE, ALL).unwrap(), b"\x1b[97u");
        assert_eq!(encode("a", SHIFT, ALL).unwrap(), b"\x1b[97;2u");
    }

    #[test]
    fn enter_tab_backspace_disambiguate_only_when_modified_or_all() {
        // Plain ones defer to legacy in disambiguate mode.
        assert_eq!(encode("enter", NONE, DIS), None);
        assert_eq!(encode("tab", NONE, DIS), None);
        assert_eq!(encode("backspace", NONE, DIS), None);
        // All-keys upgrades them.
        assert_eq!(encode("enter", NONE, ALL).unwrap(), b"\x1b[13u");
        assert_eq!(encode("tab", NONE, ALL).unwrap(), b"\x1b[9u");
        assert_eq!(encode("backspace", NONE, ALL).unwrap(), b"\x1b[127u");
        // Ctrl modified -> CSI u even in disambiguate mode.
        assert_eq!(encode("enter", CTRL, DIS).unwrap(), b"\x1b[13;5u");
    }

    #[test]
    fn navigation_and_function_keys_defer_to_legacy() {
        for key in ["up", "down", "home", "delete", "pageup", "f1", "f12"] {
            assert_eq!(encode(key, NONE, ALL), None, "{key}");
            assert_eq!(encode(key, CTRL, DIS), None, "{key}");
        }
    }
}
