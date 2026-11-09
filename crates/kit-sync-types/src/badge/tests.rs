//! Truth table ported from OB `src/ctx/tests.rs` (`badge_table`), with OB's
//! `WARN_AFTER = 3`.

use super::*;

const POLICY: BadgePolicy = BadgePolicy {
    warn_after: NonZeroU8::new(3).unwrap(),
};

fn on(running: bool, pending: bool, last: LastRun) -> BadgeInput {
    BadgeInput::On(OnState {
        activity: if running {
            Activity::Running
        } else {
            Activity::Idle
        },
        queue: if pending {
            Queue::Pending
        } else {
            Queue::Empty
        },
        last,
    })
}

fn other(streak: u32) -> LastRun {
    LastRun::Failed {
        class: FailureClass::ClearsByItself,
        streak,
    }
}

fn needs_user() -> LastRun {
    LastRun::Failed {
        class: FailureClass::NeedsUser,
        streak: 1,
    }
}

fn b(input: BadgeInput) -> SyncBadge {
    badge(input, POLICY)
}

#[test]
fn badge_table() {
    assert_eq!(b(BadgeInput::Off), SyncBadge::Off);
    assert_eq!(b(on(false, false, LastRun::Clean)), SyncBadge::Synced);
    assert_eq!(b(on(false, true, LastRun::Clean)), SyncBadge::Pending);
    assert_eq!(b(on(true, true, LastRun::Clean)), SyncBadge::Syncing);
    assert_eq!(b(on(true, false, other(3))), SyncBadge::Syncing);
    // An ordinary failure stays Pending (silent) until the streak warns.
    assert_eq!(b(on(false, true, other(1))), SyncBadge::Pending);
    assert_eq!(b(on(false, true, other(2))), SyncBadge::Pending);
    assert_eq!(b(on(false, true, other(3))), SyncBadge::Failed);
    assert_eq!(b(on(false, true, other(4))), SyncBadge::Failed);
    assert_eq!(b(on(false, true, needs_user())), SyncBadge::NeedsAction);
    assert_eq!(b(on(false, false, needs_user())), SyncBadge::NeedsAction);
    assert_eq!(b(on(false, false, other(3))), SyncBadge::Synced);
}

#[test]
fn running_wins_over_any_last_run() {
    for last in [LastRun::Clean, other(9), needs_user()] {
        for pending in [false, true] {
            assert_eq!(b(on(true, pending, last)), SyncBadge::Syncing, "{last:?}");
        }
    }
}

#[test]
fn the_policy_sets_the_threshold() {
    let at_once = BadgePolicy {
        warn_after: NonZeroU8::MIN,
    };
    assert_eq!(badge(on(false, true, other(1)), at_once), SyncBadge::Failed);
    assert_eq!(b(on(false, true, other(1))), SyncBadge::Pending);
}

#[test]
fn failed_classifies_the_app_kind() {
    #[derive(Clone, Copy)]
    struct Offline;
    impl FailureKind for Offline {
        fn class(self) -> FailureClass {
            FailureClass::ClearsByItself
        }
    }
    assert_eq!(LastRun::failed(Offline, 3), other(3));
}
