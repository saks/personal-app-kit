use kit_sync_types::SyncTrigger;

use super::FollowUp;

#[test]
fn a_manual_runs_follow_up_is_an_automatic_write() {
    let next = FollowUp::after(SyncTrigger::Manual);
    assert_eq!(next, FollowUp::Queue(SyncTrigger::Write));
}

#[test]
fn the_exit_flush_has_no_follow_up() {
    assert_eq!(FollowUp::after(SyncTrigger::Exit), FollowUp::Skip);
}

#[test]
fn every_follow_up_is_automatic() {
    let all = [
        SyncTrigger::Auto,
        SyncTrigger::Manual,
        SyncTrigger::Write,
        SyncTrigger::Flush,
        SyncTrigger::Retry,
        SyncTrigger::Online,
        SyncTrigger::Background,
        SyncTrigger::Exit,
        SyncTrigger::Settings,
    ];
    for trigger in all {
        match FollowUp::after(trigger) {
            FollowUp::Queue(next) => assert!(next.automatic(), "{trigger:?} -> {next:?}"),
            FollowUp::Skip => assert_eq!(trigger, SyncTrigger::Exit),
        }
    }
}
