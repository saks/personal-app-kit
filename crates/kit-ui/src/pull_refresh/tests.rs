use super::{MAX_PULL, THRESHOLD, pull_offset, should_trigger};

#[test]
fn pull_offset_ignores_upward_and_caps() {
    assert_eq!(pull_offset(-20.0), 0.0);
    assert_eq!(pull_offset(0.0), 0.0);
    assert!((pull_offset(100.0) - 40.0).abs() < f64::EPSILON);
    assert_eq!(pull_offset(10_000.0), MAX_PULL);
}

#[test]
fn should_trigger_at_threshold() {
    assert!(!should_trigger(THRESHOLD - 0.1));
    assert!(should_trigger(THRESHOLD));
    assert!(should_trigger(THRESHOLD + 1.0));
}
