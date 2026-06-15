//! The 256-entry terminal palette built from a scheme.

use crate::rgb::Rgb;
use crate::scheme::Scheme;

/// Component values of the standard xterm 6x6x6 color cube.
pub const CUBE_STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub colors: [Rgb; 256],
}

impl Palette {
    pub fn get(&self, index: u8) -> Rgb {
        self.colors[index as usize]
    }

    pub fn set(&mut self, index: u8, color: Rgb) {
        self.colors[index as usize] = color;
    }
}

/// Standard xterm color for indices 16..=255 (cube + grayscale ramp).
/// Indices 0..=15 fall back to the cube formula's nearest definition
/// only via [`build`]; this function is defined for 16..=255 and
/// returns black for 0..=15.
fn xterm_extended(index: u8) -> Rgb {
    if index >= 232 {
        let v = 8 + 10 * (index - 232);
        Rgb::new(v, v, v)
    } else if index >= 16 {
        let i = index - 16;
        Rgb::new(
            CUBE_STEPS[(i / 36) as usize],
            CUBE_STEPS[((i / 6) % 6) as usize],
            CUBE_STEPS[(i % 6) as usize],
        )
    } else {
        Rgb::new(0, 0, 0)
    }
}

/// Build a palette: slots 0..=15 from the scheme's ANSI colors,
/// 16..=231 the 6x6x6 cube, 232..=255 the grayscale ramp, then
/// per-index `overrides` applied on top.
pub fn build(scheme: &Scheme, overrides: &[(u8, Rgb)]) -> Palette {
    let mut colors = [Rgb::new(0, 0, 0); 256];
    colors[..16].copy_from_slice(&scheme.ansi);
    for i in 16..=255u8 {
        colors[i as usize] = xterm_extended(i);
    }
    for &(index, color) in overrides {
        colors[index as usize] = color;
    }
    Palette { colors }
}

impl Palette {
    pub fn from_scheme(scheme: &Scheme) -> Palette {
        build(scheme, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    fn palette() -> Palette {
        Palette::from_scheme(builtin::builtin("dark").unwrap())
    }

    #[test]
    fn ansi_slots_come_from_scheme() {
        let scheme = builtin::builtin("dark").unwrap();
        let p = palette();
        for i in 0..16 {
            assert_eq!(p.get(i as u8), scheme.ansi[i]);
        }
    }

    #[test]
    fn cube_known_values() {
        let p = palette();
        assert_eq!(p.get(16), "#000000".parse().unwrap());
        assert_eq!(p.get(21), "#0000ff".parse().unwrap());
        assert_eq!(p.get(46), "#00ff00".parse().unwrap());
        assert_eq!(p.get(196), "#ff0000".parse().unwrap());
        assert_eq!(p.get(231), "#ffffff".parse().unwrap());
        // 16 + 36*1 + 6*2 + 3 = 67 -> (95, 135, 175)
        assert_eq!(p.get(67), Rgb::new(95, 135, 175));
    }

    #[test]
    fn cube_uses_standard_steps() {
        let p = palette();
        for i in 16..=231u8 {
            let c = p.get(i);
            for v in [c.r, c.g, c.b] {
                assert!(CUBE_STEPS.contains(&v), "index {i} component {v}");
            }
        }
    }

    #[test]
    fn grayscale_ramp() {
        let p = palette();
        assert_eq!(p.get(232), "#080808".parse().unwrap());
        assert_eq!(p.get(244), "#808080".parse().unwrap());
        assert_eq!(p.get(255), "#eeeeee".parse().unwrap());
        for i in 232..=255u8 {
            let c = p.get(i);
            let v = 8 + 10 * (i - 232);
            assert_eq!(c, Rgb::new(v, v, v));
        }
    }

    #[test]
    fn overrides_apply_on_top() {
        let scheme = builtin::builtin("dark").unwrap();
        let red = Rgb::new(0xff, 0x00, 0x00);
        let teal = Rgb::new(0x00, 0x80, 0x80);
        let p = build(scheme, &[(0, red), (231, teal), (255, red)]);
        assert_eq!(p.get(0), red);
        assert_eq!(p.get(231), teal);
        assert_eq!(p.get(255), red);
        // Untouched indices keep their computed values.
        assert_eq!(p.get(1), scheme.ansi[1]);
        assert_eq!(p.get(232), "#080808".parse().unwrap());
    }

    #[test]
    fn later_overrides_win() {
        let scheme = builtin::builtin("dark").unwrap();
        let a = Rgb::new(1, 1, 1);
        let b = Rgb::new(2, 2, 2);
        let p = build(scheme, &[(42, a), (42, b)]);
        assert_eq!(p.get(42), b);
    }

    #[test]
    fn set_mutates() {
        let mut p = palette();
        let c = Rgb::new(9, 9, 9);
        p.set(100, c);
        assert_eq!(p.get(100), c);
    }
}
