//! The crate's own registration, as an app would write it, so the glue's
//! `extern "Rust"` symbol links in the test binary. Wasm has no items.
#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::time::Duration;

use crate::{BackgroundSync, Schedule, WorkerResult};

fn kit_test_job() -> BackgroundSync {
    BackgroundSync {
        run: |_: &Path| WorkerResult::Success,
        lib_name: crate::lib_name!("kit_test_lib"),
        log_tag: kit_core::log_tag!("kit-test"),
        schedule: Schedule {
            initial_delay: Duration::from_secs(10),
            backoff: Duration::from_secs(30),
        },
    }
}

crate::background_sync!(kit_test_job);

#[test]
fn the_macro_registers_the_job() {
    let job = __tauri_plugin_kit_android_background_sync();
    assert_eq!((job.run)(Path::new("/data/app")), WorkerResult::Success);
}
