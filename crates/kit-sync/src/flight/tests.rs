use std::sync::Arc;

use kit_sync_types::SyncTrigger;

use super::*;

fn shared(max_follow_ups: u32) -> Arc<Shared> {
    Arc::new(Shared::new(Mode::On, Gate::Ready, max_follow_ups))
}

fn start(shared: &Arc<Shared>) -> Flight {
    Shared::admit(shared, SyncTrigger::Write)
        .into_flight()
        .expect("expected an idle flight")
}

#[test]
fn one_run_queues_and_later_triggers_join_it() {
    let shared = shared(5);
    let mut flight = start(&shared);
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Flush),
        Admission::Joined
    ));
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Online),
        Admission::Joined
    ));
    assert_eq!(
        flight.next_queued(),
        Some(SyncTrigger::Flush),
        "the first one queued"
    );
    assert_eq!(flight.next_queued(), None);
    assert!(shared.idle());
}

#[test]
fn follow_ups_stop_at_the_limit() {
    let shared = shared(2);
    let mut flight = start(&shared);
    for _ in 0..2 {
        assert!(matches!(
            Shared::admit(&shared, SyncTrigger::Write),
            Admission::Joined
        ));
        assert_eq!(flight.next_queued(), Some(SyncTrigger::Write));
    }
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Write),
        Admission::Joined
    ));
    assert_eq!(flight.next_queued(), None, "the cap ends the flight");
    assert!(shared.idle());
}

#[test]
fn a_closed_gate_ends_the_flight() {
    let shared = shared(5);
    let mut flight = start(&shared);
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Write),
        Admission::Joined
    ));
    shared.needs_user(shared.epoch());
    assert_eq!(flight.next_queued(), None);
    assert!(shared.idle());
}

#[test]
fn a_dropped_flight_ends() {
    let shared = shared(5);
    let flight = start(&shared);
    assert!(!shared.idle());
    drop(flight);
    assert!(shared.idle());
}

#[test]
fn dropping_a_flight_drops_the_queued_trigger() {
    // Documented behaviour: drop ends the flight and drops what was queued.
    let shared = shared(5);
    let flight = start(&shared);
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Flush),
        Admission::Joined
    ));
    drop(flight);
    assert!(shared.idle(), "queued Flush is gone");
    let new = Shared::admit(&shared, SyncTrigger::Write)
        .into_flight()
        .expect("a new flight, not the dropped Flush");
    drop(new);
}

#[test]
fn a_panicking_flight_ends() {
    let shared = shared(5);
    let flight = start(&shared);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _flight = flight;
        panic!("the run panicked");
    }));
    assert!(panicked.is_err());
    assert!(shared.idle(), "not stuck at Running");
}

#[test]
fn an_ended_flight_leaves_the_next_one_alone() {
    let shared = shared(5);
    let mut old = start(&shared);
    assert_eq!(old.next_queued(), None);
    let _new = start(&shared);
    drop(old);
    assert!(
        !shared.idle(),
        "dropping the ended guard must not end the new flight"
    );
}

#[test]
fn the_gate_moves_only_between_ready_and_needs_user() {
    let shared = Shared::new(Mode::On, Gate::Unconfigured, 5);
    shared.needs_user(shared.epoch());
    assert!(!shared.active());
    shared.clear_needs_user(shared.epoch());
    assert!(!shared.active(), "Unconfigured stays");
    shared.set_gate(Gate::Ready);
    shared.needs_user(shared.epoch());
    assert!(!shared.active());
    shared.clear_needs_user(shared.epoch());
    assert!(shared.active());
    assert!(!Shared::new(Mode::Off, Gate::Ready, 5).active());
}

#[test]
fn a_stale_needs_user_leaves_ready_after_a_settings_save() {
    let shared = shared(5);
    let seen = shared.epoch();
    shared.set_gate(Gate::Ready);
    assert!(shared.active());
    assert!(!shared.needs_user(seen));
    assert!(shared.active());
}

#[test]
fn a_stale_success_leaves_needs_user() {
    let shared = shared(5);
    let seen = shared.epoch();
    assert!(shared.needs_user(seen));
    assert!(!shared.active());
    assert!(!shared.clear_needs_user(seen));
    assert!(!shared.active());
}

#[test]
fn a_stale_needs_user_keeps_the_queued_settings_run() {
    let shared = shared(5);
    let mut flight = start(&shared);
    let seen = shared.epoch();
    shared.set_gate(Gate::Ready);
    assert!(matches!(
        Shared::admit(&shared, SyncTrigger::Settings),
        Admission::Joined
    ));
    assert!(!shared.needs_user(seen));
    assert_eq!(flight.next_queued(), Some(SyncTrigger::Settings));
}
