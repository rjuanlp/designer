//! Public feasibility-scene contract tests.
use designer_render::{center_at, scene_at, tokens};

#[test]
fn sampling_out_of_order_does_not_change_results() {
    let expected = center_at(0.25);
    for time in [100.0, 0.0, 20.0, 0.125] {
        assert!(scene_at(time).is_some());
    }
    assert_eq!(center_at(0.25), expected);
}

#[test]
fn extreme_finite_time_does_not_generate_invalid_geometry() {
    let (x, y) = center_at(f64::MAX).expect("finite time");
    assert!(x.is_finite() && y.is_finite());
    assert!(scene_at(f64::MAX).is_some());
}

#[test]
fn quarter_period_reaches_rightmost_position() {
    let (x, _) = center_at(tokens::PERIOD / 4.0).expect("valid time");
    assert!((x - (f64::from(tokens::WIDTH) / 2.0 + tokens::AMPLITUDE)).abs() < 1e-9);
}
