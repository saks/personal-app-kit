use std::time::Duration;

use super::*;

#[test]
fn lib_names_are_bare_identifiers() {
    for good in ["sickness_tracker_lib", "budget_app_lib", "a1"] {
        assert_eq!(good.parse::<LibName>().unwrap().as_str(), good);
    }
    for (bad, why) in [
        ("", LibNameError::Empty),
        ("libfoo.so", LibNameError::FileName),
        ("foo-lib", LibNameError::InvalidChar { at: 3, ch: '-' }),
        ("1foo", LibNameError::InvalidChar { at: 0, ch: '1' }),
        ("foo bar", LibNameError::InvalidChar { at: 3, ch: ' ' }),
    ] {
        assert_eq!(bad.parse::<LibName>(), Err(why), "{bad:?}");
    }
}

#[test]
fn literals_follow_the_same_rules() {
    for raw in [
        "sickness_tracker_lib",
        "a1",
        "",
        "libfoo.so",
        "foo-lib",
        "1foo",
        "fö",
        "a🦀",
        "so",
    ] {
        assert_eq!(
            LibName::literal(raw).map(LibName::from_literal),
            raw.parse::<LibName>(),
            "{raw:?}"
        );
    }
    assert_eq!(
        LibName::literal("fö"),
        Err(LibNameError::InvalidChar { at: 1, ch: 'ö' })
    );
}

#[test]
fn the_macro_checks_at_compile_time() {
    const NAME: LibName = crate::lib_name!("budget_app_lib");
    assert_eq!(NAME, "budget_app_lib".parse().unwrap());
}

#[test]
fn the_schedule_macro_checks_at_compile_time() {
    const SCHEDULE: Schedule =
        crate::schedule!(Duration::from_millis(10_900), Duration::from_secs(30));
    assert_eq!(SCHEDULE.initial_delay(), Duration::from_millis(10_900));
    assert_eq!(SCHEDULE.backoff(), Duration::from_secs(30));
    assert_eq!(SCHEDULE.millis(), [10_900, 30_000]);
}

#[test]
fn an_initial_delay_of_zero_or_the_cap_is_kept() {
    let at_cap = Schedule::new(Schedule::MAX_INITIAL_DELAY, Schedule::MIN_BACKOFF).unwrap();
    assert_eq!(at_cap.initial_delay(), Schedule::MAX_INITIAL_DELAY);
    let immediate = Schedule::new(Duration::ZERO, Schedule::MAX_BACKOFF).unwrap();
    assert_eq!(immediate.initial_delay(), Duration::ZERO);
    assert_eq!(immediate.backoff(), Schedule::MAX_BACKOFF);
}

#[test]
fn an_initial_delay_past_the_cap_is_rejected() {
    let over = Duration::from_millis((i64::MAX / 2) as u64 + 1);
    assert_eq!(
        Schedule::new(over, Schedule::MIN_BACKOFF),
        Err(ScheduleError::InitialDelay)
    );
}

#[test]
fn backoff_must_be_from_10s_to_5h() {
    assert!(Schedule::new(Duration::ZERO, Schedule::MIN_BACKOFF).is_ok());
    assert!(Schedule::new(Duration::ZERO, Schedule::MAX_BACKOFF).is_ok());
    assert_eq!(
        Schedule::new(Duration::ZERO, Duration::from_millis(9_999)),
        Err(ScheduleError::Backoff)
    );
    assert_eq!(
        Schedule::new(Duration::ZERO, Duration::ZERO),
        Err(ScheduleError::Backoff)
    );
    assert_eq!(
        Schedule::new(
            Duration::ZERO,
            Schedule::MAX_BACKOFF + Duration::from_millis(1)
        ),
        Err(ScheduleError::Backoff)
    );
}

#[test]
fn a_fractional_millisecond_is_rejected() {
    let fractional = Duration::from_secs(10) + Duration::from_nanos(1);
    assert_eq!(
        Schedule::new(fractional, Schedule::MIN_BACKOFF),
        Err(ScheduleError::SubMillisecond)
    );
    assert_eq!(
        Schedule::new(Duration::ZERO, fractional),
        Err(ScheduleError::SubMillisecond)
    );
}
