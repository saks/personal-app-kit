use std::time::Duration;

use super::*;
use crate::config::Schedule;

fn job(run: fn(&Path) -> WorkerResult) -> BackgroundSync {
    BackgroundSync {
        run,
        lib_name: "app_lib".parse().unwrap(),
        log_tag: "app".parse().unwrap(),
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

#[test]
fn the_app_result_is_passed_through_with_the_data_dir() {
    assert_eq!(
        run(&job(needs_dir), Ok("/data/app".into())),
        WorkerResult::Failure
    );
    assert_eq!(
        run(&job(needs_dir), Ok("/elsewhere".into())),
        WorkerResult::Success
    );
}

#[test]
fn a_panic_becomes_retry() {
    assert_eq!(
        run(&job(panics), Ok("/data/app".into())),
        WorkerResult::Retry
    );
}

#[test]
fn no_data_dir_retries_without_running() {
    assert_eq!(
        run(&job(panics), Err(NoDataDir::Unreadable("null".into()))),
        WorkerResult::Retry
    );
    assert_eq!(
        run(&job(panics), Err(NoDataDir::Panicked)),
        WorkerResult::Retry
    );
}
