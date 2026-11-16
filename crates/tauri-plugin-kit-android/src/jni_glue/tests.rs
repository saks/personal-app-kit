use super::*;

#[test]
fn jni_entry_points_are_part_of_the_host_build() {
    // Name `runSync` so this fails to compile if the glue is Android-only
    // again. Do not call it: the host has no Java VM.
    let _run_sync = Java_com_saksmlz_kit_android_SyncWorker_runSync;
}

#[test]
fn enqueue_without_a_java_vm_does_not_panic() {
    assert_eq!(enqueue(), Err(EnqueueError::NoAndroidContext));
}

#[test]
fn no_android_context_displays_its_text() {
    assert_eq!(
        EnqueueError::NoAndroidContext.to_string(),
        "android context is not initialized"
    );
}

#[test]
fn a_jni_failure_displays_the_stored_text() {
    assert_eq!(
        EnqueueError::Jni("loadClass failed".to_string()).to_string(),
        "loadClass failed"
    );
}

#[test]
fn a_null_java_vm_is_the_jni_error_text() {
    let jni_err = jni::errors::Error::NullPtr("JavaVM");
    let err = jni_error(jni::errors::Error::NullPtr("JavaVM"));
    assert_eq!(err, EnqueueError::Jni(jni_err.to_string()));
    assert_eq!(err.to_string(), jni_err.to_string());
}

#[test]
fn the_registered_job_reaches_the_glue() {
    let job = registered();
    assert_eq!(job.lib_name.as_str(), "kit_test_lib");
    assert_eq!(job.log_tag.as_str(), "kit-test");
    assert_eq!(job.schedule.millis(), [10_000, 30_000]);
}
