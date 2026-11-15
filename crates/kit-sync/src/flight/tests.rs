use std::sync::Arc;

use kit_sync_types::SyncTrigger;

use super::*;

fn shared(max_follow_ups: u32) -> Arc<Shared> {
    Arc::new(Shared::new(Mode::On, Gate::Ready, max_follow_ups))
}

fn start(shared: &Arc<Shared>) -> Flight {
    match Shared::admit(shared, SyncTrigger::Write) {
        Admission::Start(flight) => flight,
        Admission::Joined => panic!("expected an idle flight"),
    }
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
    shared.needs_user();
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
    shared.needs_user();
    assert!(!shared.active());
    shared.clear_needs_user();
    assert!(!shared.active(), "Unconfigured stays");
    shared.set_gate(Gate::Ready);
    shared.needs_user();
    assert!(!shared.active());
    shared.clear_needs_user();
    assert!(shared.active());
    assert!(!Shared::new(Mode::Off, Gate::Ready, 5).active());
}
