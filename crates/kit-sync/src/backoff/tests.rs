use std::num::NonZeroU32;

use super::*;

#[test]
fn jitter_is_clamped_to_the_widest_spread() {
    assert_eq!(Jitter::new(0.9), Jitter::new(0.5));
    assert_eq!(Jitter::new(-1.0), Jitter::new(-0.5));
    assert_eq!(Jitter::new(f64::NAN), Jitter::NONE);
}

#[test]
fn spread_is_clamped_and_bounds_jitter() {
    assert_eq!(Spread::new(2.0), Spread::MAX);
    assert_eq!(Spread::new(-0.1), Spread::NONE);
    assert_eq!(Spread::new(f64::NAN), Spread::NONE);
    assert_eq!(Spread::DEFAULT.bound(Jitter::new(0.5)), Jitter::new(0.2));
    assert_eq!(Spread::NONE.bound(Jitter::new(-0.5)), Jitter::NONE);
}

#[test]
fn samples_stay_within_the_spread() {
    for _ in 0..1_000 {
        let fraction = Spread::DEFAULT.sample().fraction();
        assert!((-0.2..=0.2).contains(&fraction), "{fraction}");
    }
    assert_eq!(Spread::NONE.sample(), Jitter::NONE);
}

#[test]
fn samples_vary() {
    let first = Spread::DEFAULT.sample();
    assert!((0..100).any(|_| Spread::DEFAULT.sample() != first));
}

#[test]
fn failed_starts_at_one_and_then_increments() {
    assert_eq!(step(None, Event::Failed), Action::Arm(NonZeroU32::MIN));
    let three = NonZeroU32::new(3).unwrap();
    let four = NonZeroU32::new(4).unwrap();
    assert_eq!(step(Some(three), Event::Failed), Action::Arm(four));
    assert_eq!(
        step(Some(NonZeroU32::MAX), Event::Failed),
        Action::Arm(NonZeroU32::MAX),
        "the count saturates at u32::MAX"
    );
}

#[test]
fn debounced_resume_rearms_step_one_when_a_count_is_stored() {
    // A fired timer is the same `Some`: the decision does not look at
    // `AbortHandle::is_finished`.
    let four = NonZeroU32::new(4).unwrap();
    assert_eq!(
        step(Some(four), Event::Resumed { debounced: true }),
        Action::Arm(NonZeroU32::MIN)
    );
    assert_eq!(step(None, Event::Resumed { debounced: true }), Action::Keep);
}

#[test]
fn other_events_clear() {
    let stored = Some(NonZeroU32::MIN);
    for event in [
        Event::Write,
        Event::Online,
        Event::SettingsSaved,
        Event::Succeeded,
        Event::NeedsUser,
        Event::Resumed { debounced: false },
    ] {
        assert_eq!(step(stored, event), Action::Clear, "{event:?}");
        assert_eq!(step(None, event), Action::Clear, "{event:?}");
    }
}
