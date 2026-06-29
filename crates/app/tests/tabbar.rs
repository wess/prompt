use super::*;

#[test]
fn blend_endpoints_and_midpoint() {
    let a = Rgb::new(0, 0, 0);
    let b = Rgb::new(255, 255, 255);
    assert_eq!(blend(a, b, 0.0), a);
    assert_eq!(blend(a, b, 1.0), b);
    assert_eq!(blend(a, b, 0.5), Rgb::new(128, 128, 128));
    // Out-of-range t clamps.
    assert_eq!(blend(a, b, -1.0), a);
    assert_eq!(blend(a, b, 2.0), b);
}

#[test]
fn blend_mixes_channels_independently() {
    let a = Rgb::new(10, 200, 0);
    let b = Rgb::new(20, 100, 255);
    let m = blend(a, b, 0.1);
    assert_eq!(m, Rgb::new(11, 190, 26));
}

#[test]
fn visible_split_no_overflow_when_all_fit() {
    let (vis, over) = visible_split(3, 0, 5);
    assert_eq!(vis, vec![0, 1, 2]);
    assert!(over.is_empty());
}

#[test]
fn visible_split_reserves_a_slot_for_the_dots() {
    // 10 tabs, 5 slots: 4 inline + the `…`, so 6 overflow.
    let (vis, over) = visible_split(10, 0, 5);
    assert_eq!(vis, vec![0, 1, 2, 3]);
    assert_eq!(over, vec![4, 5, 6, 7, 8, 9]);
}

#[test]
fn visible_split_keeps_active_visible() {
    // Active tab 8 is past the inline range, so it replaces the last slot.
    let (vis, over) = visible_split(10, 8, 5);
    assert!(vis.contains(&8), "active tab must stay inline: {vis:?}");
    assert!(!over.contains(&8));
    assert_eq!(vis.len(), 4);
}

#[test]
fn fit_count_is_at_least_one() {
    assert_eq!(fit_count(-100.0), 1);
    assert_eq!(fit_count(0.0), 1);
    assert!(fit_count(2000.0) > 1);
}
