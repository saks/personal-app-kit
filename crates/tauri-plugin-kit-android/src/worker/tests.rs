use std::time::Duration;

use super::*;
use crate::config::Schedule;

fn job(run: fn(&Path) -> WorkerResult) -> BackgroundSync {
    BackgroundSync {
        init_logs: None,
        run,
        lib_name: crate::lib_name!("app_lib"),
        log_tag: kit_core::log_tag!("app"),
        schedule: Schedule {
            initial_delay: Duration::from_secs(10),
            backoff: Duration::from_secs(30),
        },
    }
}

fn needs_dir(dir: &Path) -> WorkerResult {
    if dir == Path::new("/data/app") {
        WorkerResult::Failure
    } else {
        WorkerResult::Success
    }
}

fn panics(_: &Path) -> WorkerResult {
    panic!("the app's run panicked")
}

fn needs_dir_job() -> BackgroundSync {
    job(needs_dir)
}

fn panicking_job() -> BackgroundSync {
    job(panics)
}

fn panicking_registration() -> BackgroundSync {
    panic!("the app's registration panicked")
}

#[test]
fn the_app_result_is_passed_through_with_the_data_dir() {
    assert_eq!(
        run(needs_dir_job, Ok("/data/app".into())),
        RunOutcome::Finished(WorkerResult::Failure)
    );
    assert_eq!(
        run(needs_dir_job, Ok("/elsewhere".into())),
        RunOutcome::Finished(WorkerResult::Success)
    );
}

#[test]
fn a_panic_is_reported() {
    assert_eq!(
        run(panicking_job, Ok("/data/app".into())),
        RunOutcome::Panicked
    );
}

#[test]
fn a_panic_code_matches_sync_worker_kt() {
    assert_eq!(
        RunOutcome::Panicked.code(),
        crate::result::PANIC_CODE,
        "AttemptBudget.kt's PANIC"
    );
}

#[test]
fn a_panic_in_the_registration_is_reported() {
    assert_eq!(
        run(panicking_registration, Ok("/data/app".into())),
        RunOutcome::Panicked
    );
    assert_eq!(
        run(panicking_registration, Err(NoDataDir::Panicked)),
        RunOutcome::Panicked
    );
}

#[test]
fn no_data_dir_does_not_run() {
    assert_eq!(
        run(panicking_job, Err(NoDataDir::Unreadable("null".into()))),
        RunOutcome::Finished(WorkerResult::Failure)
    );
    assert_eq!(
        run(panicking_job, Err(NoDataDir::Panicked)),
        RunOutcome::Panicked
    );
}

thread_local! {
    /// `init_logs` calls on this test's thread (tests run in parallel).
    static LOGS_READY: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn init_logs() {
    LOGS_READY.with(|calls| calls.set(calls.get() + 1));
}

/// Succeeds only if `init_logs` ran before it.
fn needs_logs(_: &Path) -> WorkerResult {
    if LOGS_READY.with(std::cell::Cell::get) > 0 {
        WorkerResult::Success
    } else {
        WorkerResult::Failure
    }
}

fn logging_job() -> BackgroundSync {
    BackgroundSync {
        init_logs: Some(init_logs),
        ..job(needs_logs)
    }
}

fn panicking_logs() {
    panic!("the app's logger panicked")
}

fn panicking_logs_job() -> BackgroundSync {
    BackgroundSync {
        init_logs: Some(panicking_logs),
        ..job(needs_logs)
    }
}

#[test]
fn init_logs_runs_once_before_the_app_run() {
    LOGS_READY.with(|calls| calls.set(0));
    assert_eq!(
        run(logging_job, Ok("/data/app".into())),
        RunOutcome::Finished(WorkerResult::Success)
    );
    assert_eq!(LOGS_READY.with(std::cell::Cell::get), 1);
}

#[test]
fn init_logs_runs_even_without_a_data_dir() {
    LOGS_READY.with(|calls| calls.set(0));
    assert_eq!(
        run(logging_job, Err(NoDataDir::Unreadable("null".into()))),
        RunOutcome::Finished(WorkerResult::Failure)
    );
    assert_eq!(
        LOGS_READY.with(std::cell::Cell::get),
        1,
        "the data-dir warning is logged"
    );
}

#[test]
fn without_init_logs_the_run_starts_directly() {
    LOGS_READY.with(|calls| calls.set(0));
    assert_eq!(
        run(|| job(needs_logs), Ok("/data/app".into())),
        RunOutcome::Finished(WorkerResult::Failure)
    );
}

#[test]
fn a_panic_in_init_logs_is_reported() {
    assert_eq!(
        run(panicking_logs_job, Ok("/data/app".into())),
        RunOutcome::Panicked
    );
}
