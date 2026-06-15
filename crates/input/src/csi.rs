//! CSI/SS3 sequence builders and xterm modifier-parameter math.

use crate::Mods;

pub(crate) const ESC: u8 = 0x1b;

/// Any encoding-relevant modifier held (cmd is rejected before this).
pub(crate) fn is_modified(mods: Mods) -> bool {
    mods.shift || mods.alt || mods.ctrl
}

/// xterm modifier parameter: 1 + (shift=1, alt=2, ctrl=4).
pub(crate) fn modifier_param(mods: Mods) -> u8 {
    let mut sum = 0;
    if mods.shift {
        sum += 1;
    }
    if mods.alt {
        sum += 2;
    }
    if mods.ctrl {
        sum += 4;
    }
    1 + sum
}

/// Unmodified cursor-class key: `CSI {final}`, or `SS3 {final}` in
/// application cursor mode.
pub(crate) fn cursor(final_byte: u8, app: bool) -> Vec<u8> {
    if app {
        ss3(final_byte)
    } else {
        vec![ESC, b'[', final_byte]
    }
}

/// Modified cursor-class key (also f1-f4): `CSI 1 ; {m} {final}`.
/// Modifiers force the CSI form even in application cursor mode.
pub(crate) fn cursor_modified(final_byte: u8, mods: Mods) -> Vec<u8> {
    let mut out = vec![ESC, b'[', b'1', b';'];
    push_num(&mut out, modifier_param(mods));
    out.push(final_byte);
    out
}

/// `SS3 {final}` (ESC O ...).
pub(crate) fn ss3(final_byte: u8) -> Vec<u8> {
    vec![ESC, b'O', final_byte]
}

/// Tilde-class key: `CSI {n} ~`, or `CSI {n} ; {m} ~` when modified.
pub(crate) fn tilde(n: u8, mods: Mods) -> Vec<u8> {
    let mut out = vec![ESC, b'['];
    push_num(&mut out, n);
    if is_modified(mods) {
        out.push(b';');
        push_num(&mut out, modifier_param(mods));
    }
    out.push(b'~');
    out
}

fn push_num(out: &mut Vec<u8>, n: u8) {
    out.extend_from_slice(n.to_string().as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(shift: bool, alt: bool, ctrl: bool) -> Mods {
        Mods {
            shift,
            alt,
            ctrl,
            cmd: false,
        }
    }

    #[test]
    fn modifier_param_table() {
        // (shift, alt, ctrl) -> 1 + shift*1 + alt*2 + ctrl*4
        let cases = [
            ((false, false, false), 1),
            ((true, false, false), 2),
            ((false, true, false), 3),
            ((true, true, false), 4),
            ((false, false, true), 5),
            ((true, false, true), 6),
            ((false, true, true), 7),
            ((true, true, true), 8),
        ];
        for ((s, a, c), want) in cases {
            assert_eq!(modifier_param(mods(s, a, c)), want, "({s},{a},{c})");
        }
    }

    #[test]
    fn cursor_forms() {
        assert_eq!(cursor(b'A', false), b"\x1b[A");
        assert_eq!(cursor(b'A', true), b"\x1bOA");
        assert_eq!(cursor_modified(b'A', mods(true, false, true)), b"\x1b[1;6A");
    }

    #[test]
    fn tilde_forms() {
        assert_eq!(tilde(3, mods(false, false, false)), b"\x1b[3~");
        assert_eq!(tilde(3, mods(false, false, true)), b"\x1b[3;5~");
        assert_eq!(tilde(15, mods(true, true, true)), b"\x1b[15;8~");
    }

    #[test]
    fn ss3_form() {
        assert_eq!(ss3(b'P'), b"\x1bOP");
    }
}
