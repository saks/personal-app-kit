use super::*;

#[test]
fn jni_entry_points_are_part_of_the_host_build() {
    // Name `runSync` so this fails to compile if the glue is Android-only
    // again. Do not call it: the host has no Java VM.
    let _run_sync = Java_com_saksmlz_kit_android_SyncWorker_runSync;
}

#[test]
fn enqueue_without_a_java_vm_does_not_panic() {
    enqueue();
}

#[test]
fn the_registered_job_reaches_the_glue() {
    let job = registered();
    assert_eq!(job.lib_name.as_str(), "kit_test_lib");
    assert_eq!(job.log_tag.as_str(), "kit-test");
    assert_eq!(job.schedule.millis(), [10_000, 30_000]);
}
