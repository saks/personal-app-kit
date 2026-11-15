use std::num::NonZeroU32;
use std::time::Duration;

use super::*;

fn n(k: u32) -> NonZeroU32 {
    NonZeroU32::new(k).expect("nonzero")
}

#[test]
fn defaults_are_todays_values() {
    let policy = SyncPolicy::default();
    assert_eq!(policy.write_delay, Duration::from_millis(1500));
    assert_eq!(policy.auto_debounce, Duration::from_secs(30));
    assert_eq!(policy.jitter, Spread::new(0.2));
    assert_eq!(policy.max_follow_ups, 5);
    assert_eq!(policy.push_budget, Duration::from_secs(15));
    assert_eq!(policy.pull_budget, Duration::from_secs(15));
    assert_eq!(policy.exit_budget, Duration::from_secs(3));
}

#[test]
fn delay_schedule_and_cap() {
    let policy = SyncPolicy::default();
    assert_eq!(
        policy.retry_delay(n(1), Jitter::NONE),
        Duration::from_secs(5)
    );
    assert_eq!(
        policy.retry_delay(n(2), Jitter::NONE),
        Duration::from_secs(15)
    );
    assert_eq!(
        policy.retry_delay(n(3), Jitter::NONE),
        Duration::from_secs(60)
    );
    assert_eq!(
        policy.retry_delay(n(4), Jitter::NONE),
        Duration::from_secs(300)
    );
    assert_eq!(
        policy.retry_delay(n(5), Jitter::NONE),
        Duration::from_secs(300)
    );
    assert_eq!(
        policy.retry_delay(n(99), Jitter::NONE),
        Duration::from_secs(300)
    );
    assert_eq!(
        policy.retry_delay(NonZeroU32::MAX, Jitter::NONE),
        Duration::from_secs(300)
    );
}

#[test]
fn jitter_is_bounded_by_the_spread_and_scales_the_delay() {
    let policy = SyncPolicy::default();
    let base = Duration::from_secs(5);
    assert_eq!(
        policy.retry_delay(NonZeroU32::MIN, Jitter::new(0.2)),
        base.mul_f64(1.2)
    );
    assert_eq!(
        policy.retry_delay(NonZeroU32::MIN, Jitter::new(-0.2)),
        base.mul_f64(0.8)
    );
    assert_eq!(
        policy.retry_delay(NonZeroU32::MIN, Jitter::new(0.5)),
        base.mul_f64(1.2),
        "clamped to ±20 %"
    );
    assert_eq!(
        policy.retry_delay(NonZeroU32::MIN, Jitter::new(-1.0)),
        base.mul_f64(0.8)
    );
}

#[test]
fn a_single_step_repeats() {
    let steps = RetrySteps::new(Duration::from_secs(7), []);
    assert_eq!(steps.step(n(1)), Duration::from_secs(7));
    assert_eq!(steps.step(n(9)), Duration::from_secs(7));
}

#[test]
fn a_huge_step_does_not_overflow() {
    let policy = SyncPolicy {
        retry_steps: RetrySteps::new(Duration::MAX, []),
        ..SyncPolicy::default()
    };
    assert_eq!(
        policy.retry_delay(NonZeroU32::MIN, Jitter::new(0.2)),
        Duration::MAX
    );
}
