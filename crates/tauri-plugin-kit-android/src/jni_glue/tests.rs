use std::time::Duration;

use super::*;

#[test]
fn jni_entry_points_are_part_of_the_host_build() {
    // Name the symbols so this fails to compile if the glue is Android-only
    // again. Do not call them: the host has no Java VM.
    let _enqueue: fn() = enqueue;
    let _run_sync = Java_com_saksmlz_kit_android_SyncWorker_runSync;
}

#[test]
fn the_registered_job_reaches_the_glue() {
    let job = registered();
    assert_eq!(job.lib_name.as_str(), "kit_test_lib");
    assert_eq!(job.log_tag.as_str(), "kit-test");
}

#[test]
fn the_schedule_goes_to_kotlin_in_whole_seconds() {
    let schedule = Schedule {
        initial_delay: Duration::from_millis(10_900),
        backoff: Duration::from_secs(30),
    };
    assert_eq!(seconds(schedule), [10, 30]);
    let huge = Schedule {
        initial_delay: Duration::MAX,
        backoff: Duration::ZERO,
    };
    assert_eq!(seconds(huge), [i64::MAX, 0]);
}
