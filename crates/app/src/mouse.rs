//! Pure mouse policy: when to report to the pty, how clicks map to
//! selection modes, how wheel deltas become whole lines. No gpui types so
//! everything here is unit-testable.

use input::MouseButton;
use vt::{MouseMode, SelectionMode};

/// Cross-event pointer state, owned by the view and shared with the
/// element's per-frame event closures.
#[derive(Debug, Default)]
pub struct MouseState {
    /// A left-button selection gesture is in progress.
    pub selecting: bool,
    /// The selection gesture extended past its starting cell (or began as
    /// a multi-click), so it survives release.
    pub dragged: bool,
    /// Cell where the selection press landed, to tell click from drag.
    pub pressed: Option<(usize, usize)>,
    /// Button held while reporting to the pty (press sent, release owed).
    pub report_button: Option<MouseButton>,
    /// Last cell reported as motion, for coalescing duplicates.
    pub last_motion: Option<(usize, usize)>,
    /// Fractional wheel lines carried between scroll events.
    pub wheel: f32,
}

/// Whether mouse events go to the pty instead of driving selection.
/// Shift always reclaims the mouse for the terminal user.
pub fn reports(mode: MouseMode, shift: bool) -> bool {
    mode != MouseMode::None && !shift
}

/// Whether a motion event is reported, given the button currently held:
/// Click mode never reports motion, Drag only while a button is down,
/// Motion always.
pub fn reports_motion(mode: MouseMode, held: Option<MouseButton>) -> bool {
    match mode {
        MouseMode::Motion => true,
        MouseMode::Drag => held.is_some(),
        MouseMode::Click | MouseMode::None => false,
    }
}

/// Click count to selection mode: single = cell, double = word, triple =
/// line; further rapid clicks cycle.
pub fn click_mode(count: usize) -> SelectionMode {
    match (count.max(1) - 1) % 3 {
        0 => SelectionMode::Cell,
        1 => SelectionMode::Word,
        _ => SelectionMode::Line,
    }
}

/// Where a wheel event goes. Mouse reporting wins (shift bypasses), then
/// alternate scroll on the alt screen, else the display scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelRoute {
    /// Encode wheel presses for the pty.
    Report,
    /// Synthesize arrow keys (alt screen + DECSET 1007).
    Arrows,
    /// Scroll the viewport through scrollback.
    Display,
}

pub fn route_wheel(mode: MouseMode, shift: bool, alt_screen: bool, alt_scroll: bool) -> WheelRoute {
    if reports(mode, shift) {
        WheelRoute::Report
    } else if alt_screen && alt_scroll && !shift {
        WheelRoute::Arrows
    } else {
        WheelRoute::Display
    }
}

/// Fold a wheel delta (in lines, possibly fractional — trackpad pixel
/// deltas divided by the cell height) into the accumulator and take out
/// the whole lines. A direction change drops the leftover fraction so
/// reversals respond immediately.
pub fn wheel_lines(acc: &mut f32, delta: f32) -> i32 {
    if *acc != 0.0 && acc.signum() != delta.signum() && delta != 0.0 {
        *acc = 0.0;
    }
    *acc += delta;
    let whole = acc.trunc();
    *acc -= whole;
    whole as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporting_requires_mode_and_no_shift() {
        assert!(!reports(MouseMode::None, false));
        assert!(reports(MouseMode::Click, false));
        assert!(reports(MouseMode::Drag, false));
        assert!(reports(MouseMode::Motion, false));
        // Shift always bypasses reporting.
        assert!(!reports(MouseMode::Motion, true));
        assert!(!reports(MouseMode::Click, true));
    }

    #[test]
    fn motion_reporting_per_mode() {
        let held = Some(MouseButton::Left);
        assert!(!reports_motion(MouseMode::None, held));
        assert!(!reports_motion(MouseMode::Click, held));
        assert!(reports_motion(MouseMode::Drag, held));
        assert!(!reports_motion(MouseMode::Drag, None));
        assert!(reports_motion(MouseMode::Motion, held));
        assert!(reports_motion(MouseMode::Motion, None));
    }

    #[test]
    fn click_counts_map_to_modes() {
        assert_eq!(click_mode(0), SelectionMode::Cell); // defensive
        assert_eq!(click_mode(1), SelectionMode::Cell);
        assert_eq!(click_mode(2), SelectionMode::Word);
        assert_eq!(click_mode(3), SelectionMode::Line);
        // Rapid clicking cycles.
        assert_eq!(click_mode(4), SelectionMode::Cell);
        assert_eq!(click_mode(5), SelectionMode::Word);
        assert_eq!(click_mode(6), SelectionMode::Line);
    }

    #[test]
    fn wheel_routing_precedence() {
        use WheelRoute::*;
        // Reporting wins over everything when active and shift is up.
        assert_eq!(route_wheel(MouseMode::Click, false, true, true), Report);
        assert_eq!(route_wheel(MouseMode::Motion, false, false, false), Report);
        // Shift bypasses reporting; alt scroll applies on the alt screen.
        assert_eq!(route_wheel(MouseMode::Motion, true, true, true), Display);
        assert_eq!(route_wheel(MouseMode::None, false, true, true), Arrows);
        // Alt scroll needs both the alt screen and the mode.
        assert_eq!(route_wheel(MouseMode::None, false, true, false), Display);
        assert_eq!(route_wheel(MouseMode::None, false, false, true), Display);
        assert_eq!(route_wheel(MouseMode::None, false, false, false), Display);
    }

    #[test]
    fn wheel_lines_accumulates_fractions() {
        let mut acc = 0.0;
        assert_eq!(wheel_lines(&mut acc, 0.4), 0);
        assert_eq!(wheel_lines(&mut acc, 0.4), 0);
        assert_eq!(wheel_lines(&mut acc, 0.4), 1); // 1.2 -> 1, carry 0.2
        assert!((acc - 0.2).abs() < 1e-6);
        assert_eq!(wheel_lines(&mut acc, 2.0), 2);
    }

    #[test]
    fn wheel_lines_whole_deltas_pass_through() {
        let mut acc = 0.0;
        assert_eq!(wheel_lines(&mut acc, 3.0), 3);
        assert_eq!(wheel_lines(&mut acc, -2.0), -2);
        assert_eq!(acc, 0.0);
    }

    #[test]
    fn wheel_lines_direction_change_drops_fraction() {
        let mut acc = 0.0;
        assert_eq!(wheel_lines(&mut acc, 0.9), 0);
        // Reversing direction must not fight the stale +0.9.
        assert_eq!(wheel_lines(&mut acc, -1.0), -1);
        assert_eq!(acc, 0.0);
    }
}
