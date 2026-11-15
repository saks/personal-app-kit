//! Scheduler tests on a paused clock: ported from octo-budget-rs
//! `autosync/tests.rs` (`2ebfd24`), plus the four sickness_tracker pieces.

mod fake;

use std::time::Duration;

use kit_sync_types::{FailureClass, SyncTrigger};
use tokio::time::Instant;

use self::fake::{Behavior, Fake, FakeError, Report, Seen, advance, offline};
use crate::*;

fn synced(trigger: SyncTrigger, local_changes: u64) -> Seen {
    Seen::Synced(
        trigger,
        Report {
            local_changes,
            left_behind: 0,
        },
    )
}

// --- Ported from OB: single flight, write timer, debounce, suspend, exit. ---

#[tokio::test(start_paused = true)]
async fn three_writes_within_the_window_make_one_run() {
    let host = Fake::ready();
    schedule_after_write(&host);
    advance(500).await;
    schedule_after_write(&host);
    advance(500).await;
    schedule_after_write(&host);
    advance(1400).await;
    assert!(host.started().is_empty(), "timer restarts on every write");
    advance(5_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
}

#[tokio::test(start_paused = true)]
async fn writes_during_a_run_queue_exactly_one_more_run() {
    let host = Fake::new(
        Mode::On,
        Gate::Ready,
        Behavior::Succeed {
            takes: Duration::from_secs(10),
        },
    );
    schedule_after_write(&host);
    advance(1_600).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    // Two more saves while the first run holds the lock.
    host.set_pending(1);
    schedule_after_write(&host);
    advance(1_600).await;
    schedule_after_write(&host);
    advance(1_600).await;
    assert_eq!(host.started().len(), 1, "still running the first");
    advance(30_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Write]);
    assert_eq!(
        host.finished()[1],
        synced(SyncTrigger::Write, 1),
        "the queued run pushes the row saved during the first run"
    );
    assert!(host.auto().shared.idle());
}

#[tokio::test(start_paused = true)]
async fn auto_is_debounced() {
    let host = Fake::ready();
    on_first_load(&host);
    advance(1_000).await;
    on_resumed(&host);
    on_first_load(&host);
    advance(1_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Auto]);
    advance(30_000).await;
    on_resumed(&host);
    advance(1_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Auto, SyncTrigger::Auto]);
}

#[tokio::test(start_paused = true)]
async fn write_and_flush_are_not_debounced() {
    let host = Fake::ready();
    for _ in 0..2 {
        schedule_after_write(&host);
        advance(2_000).await;
    }
    for _ in 0..2 {
        host.set_pending(1);
        on_suspended(&host);
        advance(1_000).await;
    }
    assert_eq!(
        host.started(),
        vec![
            SyncTrigger::Write,
            SyncTrigger::Write,
            SyncTrigger::Flush,
            SyncTrigger::Flush
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn suspend_with_nothing_unpushed_does_not_sync() {
    let host = Fake::ready();
    host.set_pending(0);
    on_suspended(&host);
    advance(5_000).await;
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn suspend_drops_the_write_timer_and_flushes() {
    let host = Fake::ready();
    schedule_after_write(&host);
    advance(500).await;
    on_suspended(&host);
    advance(5_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Flush]);
}

#[tokio::test(start_paused = true)]
async fn exit_returns_within_budget_against_a_hanging_sync() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::Hang);
    let budget = host.auto().policy().exit_budget;
    let started = Instant::now();
    let flushed = flush_before_exit(&host).await;
    let took = started.elapsed();
    assert_eq!(flushed, Err(BudgetExceeded { budget }));
    assert!(took >= budget && took < budget + Duration::from_millis(100));
    assert_eq!(host.started(), vec![SyncTrigger::Exit]);
}

#[tokio::test(start_paused = true)]
async fn exit_with_nothing_unpushed_skips_the_sync() {
    let host = Fake::ready();
    host.set_pending(0);
    assert_eq!(flush_before_exit(&host).await, Ok(()));
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn exit_drops_the_write_timer() {
    let host = Fake::ready();
    schedule_after_write(&host);
    assert_eq!(flush_before_exit(&host).await, Ok(()));
    advance(5_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Exit], "no Write after it");
}

async fn assert_no_automatic_activity(host: &Fake) {
    schedule_after_write(host);
    assert!(!host.auto().scheduled(), "no timer armed");
    on_first_load(host);
    on_resumed(host);
    on_suspended(host);
    on_online(host);
    assert_eq!(flush_before_exit(host).await, Ok(()));
    advance(60_000).await;
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn mode_off_arms_no_timers_and_starts_no_runs() {
    let host = Fake::new(
        Mode::Off,
        Gate::Ready,
        Behavior::Succeed {
            takes: Duration::ZERO,
        },
    );
    assert_no_automatic_activity(&host).await;
    // Manual still works and still reports.
    run(&host, SyncTrigger::Manual).await.expect("manual");
    assert_eq!(host.started(), vec![SyncTrigger::Manual]);
    assert_eq!(host.finished().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn unconfigured_sync_arms_no_timers_and_starts_no_runs() {
    let host = Fake::new(
        Mode::On,
        Gate::Unconfigured,
        Behavior::Succeed {
            takes: Duration::ZERO,
        },
    );
    assert_no_automatic_activity(&host).await;
}

// --- Ported from OB: the NeedsUser gate and settings_saved. ---

#[tokio::test(start_paused = true)]
async fn needs_user_stops_automatic_runs_until_settings_are_saved() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::RejectToken);
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    assert_eq!(
        host.finished()[0],
        Seen::Failed(
            SyncTrigger::Write,
            FailureClass::NeedsUser,
            FakeError::RejectToken
        )
    );
    schedule_after_write(&host);
    on_resumed(&host);
    on_suspended(&host);
    advance(60_000).await;
    assert_eq!(host.started().len(), 1, "blocked until Settings are saved");

    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    settings_saved(&host, Gate::Ready);
    advance(1).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write, SyncTrigger::Settings],
        "saving Settings with a pending row starts a Settings run"
    );
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(
        host.started(),
        vec![
            SyncTrigger::Write,
            SyncTrigger::Settings,
            SyncTrigger::Write
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn a_manual_success_reopens_automatic_runs() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::RejectToken);
    run(&host, SyncTrigger::Manual).await.expect_err("401");
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Manual], "blocked");

    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    run(&host, SyncTrigger::Manual).await.expect("manual");
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Manual, SyncTrigger::Manual, SyncTrigger::Write]
    );
}

#[tokio::test(start_paused = true)]
async fn failures_that_clear_by_themselves_keep_automatic_runs_on() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert!(host.auto().active(), "the gate stays Ready");
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Write]);
}

#[tokio::test(start_paused = true)]
async fn settings_saved_with_pending_starts_a_settings_run() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::RejectToken);
    schedule_after_write(&host);
    advance(2_000).await;
    assert!(matches!(
        host.finished()[0],
        Seen::Failed(_, FailureClass::NeedsUser, _)
    ));
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    host.set_pending(1);
    settings_saved(&host, Gate::Ready);
    advance(1).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write, SyncTrigger::Settings]
    );
    assert!(matches!(host.finished().last(), Some(Seen::Synced(..))));
    assert_eq!(host.pending_now(), 0);
}

#[tokio::test(start_paused = true)]
async fn settings_saved_with_nothing_pending_does_nothing() {
    let host = Fake::ready();
    host.set_pending(0);
    settings_saved(&host, Gate::Ready);
    advance(1).await;
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn settings_saved_unconfigured_turns_automatic_runs_off() {
    let host = Fake::ready();
    settings_saved(&host, Gate::Unconfigured);
    advance(1).await;
    assert!(!host.auto().active());
    schedule_after_write(&host);
    advance(5_000).await;
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn settings_saved_drops_an_armed_retry() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.auto().failures(), 1);
    host.set_pending(0);
    settings_saved(&host, Gate::Ready);
    advance(1).await;
    assert_eq!(host.auto().failures(), 0);
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    advance(60_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write],
        "the old retry never fires"
    );
}

#[tokio::test(start_paused = true)]
async fn settings_saved_returns_while_the_engine_lock_is_held() {
    // The app may hold its engine lock across `settings_saved`. The Settings
    // run must spawn and wait, not take the lock on this stack.
    let host = Fake::ready();
    host.set_pending(1);
    let hold = host.0.hold.clone();
    let held = hold.lock().await;
    settings_saved(&host, Gate::Ready);
    advance(50).await;
    assert!(
        host.started().is_empty(),
        "spawned Settings is waiting on the lock; started={:?}",
        host.started()
    );
    drop(held);
    advance(1).await;
    assert_eq!(host.started(), vec![SyncTrigger::Settings]);
}

// --- Ported from OB: backoff, retry and Online. ---

#[tokio::test(start_paused = true)]
async fn failures_that_clear_by_themselves_retry_on_the_schedule() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    // The Write fired at 1.5 s and failed at once: the retry is due at 6.5 s.
    advance(4_499).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write], "before 5 s");
    advance(2).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Retry]);
    // The Retry already failed and armed the next one.
    assert_eq!(host.auto().failures(), 2);

    advance(14_998).await;
    assert_eq!(host.started().len(), 2, "before 15 s");
    advance(2).await;
    assert_eq!(host.started().len(), 3);
    assert_eq!(host.started()[2], SyncTrigger::Retry);
    assert_eq!(host.auto().failures(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_jittered_retry_stays_within_the_spread() {
    let host = Fake::with_policy(
        SyncPolicy::default(),
        Mode::On,
        Gate::Ready,
        Behavior::Offline,
    );
    schedule_after_write(&host);
    advance(1_501).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    advance(3_998).await;
    assert_eq!(host.started().len(), 1, "not before 5 s − 20 %");
    advance(2_002).await;
    assert_eq!(host.started().len(), 2, "by 5 s + 20 %");
}

#[tokio::test(start_paused = true)]
async fn retry_skips_when_nothing_is_pending() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    host.set_pending(0);
    advance(6_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
}

#[tokio::test(start_paused = true)]
async fn needs_user_never_retries() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::RejectToken);
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    assert_eq!(host.auto().failures(), 0);
    advance(60_000).await;
    assert_eq!(host.started().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn needs_user_drops_an_armed_retry() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.auto().failures(), 1);
    host.behave(Behavior::RejectToken);
    host.set_pending(1);
    run(&host, SyncTrigger::Manual).await.expect_err("401");
    assert_eq!(
        host.auto().failures(),
        0,
        "NeedsUser clears the armed retry"
    );
    advance(60_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write, SyncTrigger::Manual],
        "the old retry never fires"
    );
}

#[tokio::test(start_paused = true)]
async fn manual_failure_does_not_arm_a_retry() {
    let host = offline();
    run(&host, SyncTrigger::Manual).await.expect_err("offline");
    assert_eq!(host.auto().failures(), 0);
    advance(60_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Manual]);
}

#[tokio::test(start_paused = true)]
async fn write_resets_the_backoff() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.auto().failures(), 1);
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    host.set_pending(1);
    schedule_after_write(&host);
    assert_eq!(host.auto().failures(), 0);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Write]);
}

#[tokio::test(start_paused = true)]
async fn resume_resets_the_backoff_and_runs_auto() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert!(host.auto().failures() > 0);
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    host.set_pending(1);
    on_resumed(&host);
    advance(1).await;
    assert_eq!(host.auto().failures(), 0);
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Auto]);
}

#[tokio::test(start_paused = true)]
async fn a_debounced_resume_rearms_the_retry_at_the_first_step() {
    let host = offline();
    on_resumed(&host);
    advance(1).await;
    assert_eq!(host.started(), vec![SyncTrigger::Auto]);
    advance(20_000).await;
    assert_eq!(host.auto().failures(), 3, "5 s and 15 s retries failed");
    // Inside the 30 s window: no Auto, but the retry comes back in 5 s.
    on_resumed(&host);
    assert_eq!(host.auto().failures(), 1);
    assert!(host.auto().scheduled());
    let before = host.started().len();
    advance(4_999).await;
    assert_eq!(host.started().len(), before);
    advance(2).await;
    assert_eq!(host.started().last(), Some(&SyncTrigger::Retry));
}

#[tokio::test(start_paused = true)]
async fn online_with_pending_starts_a_run_right_away() {
    let host = Fake::ready();
    on_online(&host);
    advance(1).await;
    assert_eq!(host.started(), vec![SyncTrigger::Online]);
}

#[tokio::test(start_paused = true)]
async fn online_is_not_debounced() {
    let host = Fake::ready();
    for _ in 0..2 {
        host.set_pending(1);
        on_online(&host);
        advance(1_000).await;
    }
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Online, SyncTrigger::Online]
    );
}

#[tokio::test(start_paused = true)]
async fn online_with_nothing_pending_does_nothing() {
    let host = Fake::ready();
    host.set_pending(0);
    on_online(&host);
    advance(1).await;
    assert!(host.started().is_empty());
}

#[tokio::test(start_paused = true)]
async fn online_resets_an_armed_retry() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    assert!(host.auto().failures() > 0);
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    host.set_pending(1);
    on_online(&host);
    advance(1).await;
    assert_eq!(host.auto().failures(), 0);
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write, SyncTrigger::Online]
    );
}

#[tokio::test(start_paused = true)]
async fn mode_off_arms_no_retry() {
    let host = Fake::new(Mode::Off, Gate::Ready, Behavior::Offline);
    schedule_after_write(&host);
    on_online(&host);
    advance(60_000).await;
    assert!(host.started().is_empty());
    assert_eq!(host.auto().failures(), 0);
}

// --- Added from ST (1): the follow-up cap. ---

#[tokio::test(start_paused = true)]
async fn follow_ups_stop_at_the_limit() {
    let host = Fake::ready();
    host.0
        .leave_behind
        .store(u64::MAX, std::sync::atomic::Ordering::SeqCst);
    schedule_after_write(&host);
    advance(60_000).await;
    let cap = host.auto().policy().max_follow_ups;
    assert_eq!(host.started().len(), 1 + usize::try_from(cap).expect("cap"));
    assert!(host.auto().shared.idle(), "the flight let go");
    // The next save starts a flight of its own.
    host.0
        .leave_behind
        .store(0, std::sync::atomic::Ordering::SeqCst);
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started().len(), 2 + usize::try_from(cap).expect("cap"));
}

#[tokio::test(start_paused = true)]
async fn queued_writes_also_count_towards_the_limit() {
    let policy = SyncPolicy {
        max_follow_ups: 1,
        ..fake::exact_policy()
    };
    let host = Fake::with_policy(
        policy,
        Mode::On,
        Gate::Ready,
        Behavior::Succeed {
            takes: Duration::from_secs(10),
        },
    );
    schedule_after_write(&host);
    advance(2_000).await;
    // Queue behind the first run, then behind the follow-up.
    schedule_after_write(&host);
    advance(10_000).await;
    schedule_after_write(&host);
    advance(60_000).await;
    assert_eq!(host.started().len(), 2, "one follow-up, then the cap");
    assert!(host.auto().shared.idle());
}

// --- Added from ST (2): a flight ends on panic or abort. ---

#[tokio::test(start_paused = true)]
async fn a_panicking_run_leaves_the_flight_idle() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::Panic);
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    assert!(host.auto().shared.idle(), "not stuck at Running");
    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Write]);
}

#[tokio::test(start_paused = true)]
async fn an_aborted_run_leaves_the_flight_idle() {
    let host = Fake::new(Mode::On, Gate::Ready, Behavior::Hang);
    let Admission::Start(flight) = host.auto().admit(SyncTrigger::Background) else {
        panic!("idle");
    };
    let job = host.clone();
    let task = tokio::spawn(async move {
        let _flight = flight;
        run(&job, SyncTrigger::Background).await
    });
    advance(1_000).await;
    assert!(matches!(
        host.auto().admit(SyncTrigger::Write),
        Admission::Joined
    ));
    task.abort();
    assert!(task.await.is_err_and(|err| err.is_cancelled()));
    assert!(host.auto().shared.idle(), "not stuck at Running");
}

/// Dropping the runtime aborts every task on it, including the `drive` task
/// that `request` spawned and that owns the flight (the test never holds it).
#[test]
fn an_aborted_drive_task_leaves_the_flight_idle() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .expect("runtime");
    let host = rt.block_on(async {
        let host = Fake::new(Mode::On, Gate::Ready, Behavior::Hang);
        schedule_after_write(&host);
        advance(2_000).await;
        host
    });
    assert_eq!(host.started(), vec![SyncTrigger::Write]);
    assert!(
        !host.auto().shared.idle(),
        "the drive task holds the flight"
    );
    drop(rt);
    assert!(host.auto().shared.idle(), "not stuck at Running");
}

// --- Added from ST (3): an engine-requested follow-up. ---

#[tokio::test(start_paused = true)]
async fn rows_left_behind_get_exactly_one_follow_up() {
    let host = Fake::ready();
    host.0
        .leave_behind
        .store(1, std::sync::atomic::Ordering::SeqCst);
    schedule_after_write(&host);
    advance(10_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Write, SyncTrigger::Write]);
    assert!(host.auto().shared.idle());
}

#[tokio::test(start_paused = true)]
async fn a_follow_up_during_a_queued_run_joins_it() {
    let host = Fake::new(
        Mode::On,
        Gate::Ready,
        Behavior::Succeed {
            takes: Duration::from_secs(10),
        },
    );
    host.0
        .leave_behind
        .store(1, std::sync::atomic::Ordering::SeqCst);
    schedule_after_write(&host);
    advance(2_000).await;
    schedule_after_write(&host);
    advance(60_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Write, SyncTrigger::Write],
        "the queued Write covers the engine's follow-up"
    );
}

#[tokio::test(start_paused = true)]
async fn a_manual_run_leaving_rows_behind_queues_one_automatic_run() {
    let host = Fake::ready();
    host.0
        .leave_behind
        .store(1, std::sync::atomic::Ordering::SeqCst);
    run(&host, SyncTrigger::Manual).await.expect("manual");
    advance(1_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Manual, SyncTrigger::Write],
        "the follow-up is automatic, not a second Manual start"
    );
    assert!(host.auto().shared.idle());
}

#[tokio::test(start_paused = true)]
async fn a_manual_runs_follow_up_failing_offline_arms_a_retry() {
    let host = Fake::ready();
    host.0
        .leave_behind
        .store(1, std::sync::atomic::Ordering::SeqCst);
    // Scripted per sync, so the result doesn't depend on when the spawned
    // follow-up runs: the Manual run succeeds, the follow-up is offline.
    // The later unscripted Retry uses `ready()`'s Succeed.
    host.script([
        Behavior::Succeed {
            takes: Duration::ZERO,
        },
        Behavior::Offline,
    ]);
    run(&host, SyncTrigger::Manual).await.expect("manual");
    advance(1_000).await;
    assert_eq!(
        host.finished().last(),
        Some(&Seen::Failed(
            SyncTrigger::Write,
            FailureClass::ClearsByItself,
            FakeError::Offline
        ))
    );
    assert_eq!(host.auto().failures(), 1);
    assert!(host.auto().scheduled(), "the retry is armed");
    host.set_pending(1);
    advance(10_000).await;
    assert_eq!(
        host.started(),
        vec![SyncTrigger::Manual, SyncTrigger::Write, SyncTrigger::Retry]
    );
}

#[tokio::test(start_paused = true)]
async fn the_exit_flush_queues_no_follow_up() {
    let host = Fake::ready();
    host.0
        .leave_behind
        .store(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(flush_before_exit(&host).await, Ok(()));
    advance(10_000).await;
    assert_eq!(host.started(), vec![SyncTrigger::Exit]);
    assert!(host.auto().shared.idle(), "no flight during shutdown");
}

#[tokio::test(start_paused = true)]
async fn the_default_host_never_asks_for_a_follow_up() {
    #[derive(Clone)]
    struct Plain(Fake);
    impl Host for Plain {
        type Report = Report;
        type Error = FakeError;
        fn autosync(&self) -> Option<&AutoSync> {
            self.0.autosync()
        }
        async fn pending(&self) -> Result<u64, FakeError> {
            self.0.pending().await
        }
        async fn sync(&self) -> Result<Report, FakeError> {
            self.0.sync().await
        }
        fn emit(&self, event: &SyncEvent<'_, Report, FakeError>) {
            self.0.emit(event);
        }
        fn class(&self, error: &FakeError) -> FailureClass {
            self.0.class(error)
        }
    }
    let fake = Fake::ready();
    fake.0
        .leave_behind
        .store(u64::MAX, std::sync::atomic::Ordering::SeqCst);
    let host = Plain(fake.clone());
    schedule_after_write(&host);
    advance(10_000).await;
    assert_eq!(fake.started(), vec![SyncTrigger::Write]);
}

// --- Added from ST (4): failures() and scheduled(). ---

#[tokio::test(start_paused = true)]
async fn status_queries_track_arm_fire_and_reset() {
    let host = Fake::new(
        Mode::On,
        Gate::Ready,
        Behavior::Succeed {
            takes: Duration::from_secs(1),
        },
    );
    assert!(!host.auto().scheduled());
    schedule_after_write(&host);
    assert!(host.auto().scheduled(), "write timer armed");
    advance(2_000).await;
    assert!(!host.auto().scheduled(), "write timer fired");
    assert_eq!(host.auto().failures(), 0);

    host.behave(Behavior::Offline);
    host.set_pending(1);
    schedule_after_write(&host);
    advance(2_000).await;
    assert_eq!(host.auto().failures(), 1);
    assert!(host.auto().scheduled(), "retry armed");
    advance(5_000).await;
    assert_eq!(host.auto().failures(), 2, "the retry fired and failed");
    assert!(host.auto().scheduled(), "the next retry is armed");

    host.behave(Behavior::Succeed {
        takes: Duration::ZERO,
    });
    run(&host, SyncTrigger::Manual).await.expect("manual");
    assert_eq!(host.auto().failures(), 0, "a success resets");
    assert!(!host.auto().scheduled());
}

#[tokio::test(start_paused = true)]
async fn a_skipped_retry_is_no_longer_scheduled() {
    let host = offline();
    schedule_after_write(&host);
    advance(2_000).await;
    host.set_pending(0);
    advance(6_000).await;
    assert_eq!(host.auto().failures(), 1, "still failing");
    assert!(!host.auto().scheduled(), "nothing armed");
}
