use std::time::Duration;

use super::*;
use crate::config::Schedule;

fn job(run: fn(&Path) -> WorkerResult) -> BackgroundSync {
    BackgroundSync {
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
        WorkerResult::Failure
    );
    assert_eq!(
        run(needs_dir_job, Ok("/elsewhere".into())),
        WorkerResult::Success
    );
}

#[test]
fn a_panic_becomes_retry() {
    assert_eq!(
        run(panicking_job, Ok("/data/app".into())),
        WorkerResult::Retry
    );
}

#[test]
fn a_panic_in_the_registration_becomes_retry() {
    assert_eq!(
        run(panicking_registration, Ok("/data/app".into())),
        WorkerResult::Retry
    );
    assert_eq!(
        run(panicking_registration, Err(NoDataDir::Panicked)),
        WorkerResult::Retry
    );
}

#[test]
fn no_data_dir_retries_without_running() {
    assert_eq!(
        run(panicking_job, Err(NoDataDir::Unreadable("null".into()))),
        WorkerResult::Retry
    );
    assert_eq!(
        run(panicking_job, Err(NoDataDir::Panicked)),
        WorkerResult::Retry
    );
}
