//! The shared Android WorkManager background-sync job (sickness_tracker#96).
//!
//! The Android library under `android/` carries `SyncWorker`
//! (`com.saksmlz.kit.android`), so the JNI symbol of `runSync` is fixed by
//! this crate and no app writes a `Java_com_saksmlz_…` function. The app's
//! native library name, log tag and schedule travel in the WorkManager input
//! `Data`.
//!
//! - The app registers its job once, with `background_sync!`. That links
//!   a function the worker calls; it is not a runtime `init`, because in a
//!   cold process WorkManager starts the worker before any app code runs.
//! - `enqueue` (Android, and host tests): one network-constrained job,
//!   `REPLACE`, so at most one is queued. A cold worker may call it: the
//!   context comes from `SyncWorker`, not from a Tauri Activity. An in-app
//!   call still needs the Activity to have started. A missing context is
//!   logged, not a panic.
//! - In a cold process the worker needs no Tauri runtime: it loads the app's
//!   library, calls `BackgroundSync::init_logs` (if set), then
//!   `BackgroundSync::run` with the data dir. A panic, in the app's
//!   registration, its `init_logs` or its run, becomes `WorkerResult::Retry`.
//!   A library that cannot be loaded fails the job (`SyncWorker` does not
//!   retry an `UnsatisfiedLinkError`).
//!
//! Device test steps for adopting apps: `docs/android-background-sync-device-test.md`.
//!
//! Wasm builds get no items: the crate is Android-only, and is in the kit
//! workspace (whose gate also builds `wasm32`) only for its host tests.

#[cfg(not(target_arch = "wasm32"))]
mod config;
#[cfg(not(target_arch = "wasm32"))]
mod result;

// The worker side: Android, plus host tests so `make check` builds the glue.
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod jni_glue;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod process;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod registry;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod worker;

#[cfg(not(target_arch = "wasm32"))]
pub use config::{BackgroundSync, LibName, LibNameError, LibNameLiteral, Schedule, ScheduleError};
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
pub use jni_glue::enqueue;
#[cfg(not(target_arch = "wasm32"))]
pub use result::WorkerResult;

/// Registers the app's background job. Call it once, at the top level of the
/// app's library crate, with a `fn() -> BackgroundSync`. Build the names
/// with `lib_name!` and `kit_core::log_tag!`, and the delays with
/// `schedule!`. All three are checked at compile time, so the function has
/// no failure path:
///
/// ```ignore
/// use std::time::Duration;
/// use tauri_plugin_kit_android::{BackgroundSync, lib_name, schedule};
///
/// fn background_sync() -> BackgroundSync {
///     BackgroundSync {
///         init_logs: Some(init_logs),
///         run: run_blocking,
///         lib_name: lib_name!("sickness_tracker_lib"),
///         log_tag: kit_core::log_tag!("sickness-tracker"),
///         schedule: schedule!(Duration::from_secs(10), Duration::from_secs(30)),
///     }
/// }
/// tauri_plugin_kit_android::background_sync!(background_sync);
/// ```
#[macro_export]
macro_rules! background_sync {
    ($config:path) => {
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub fn __tauri_plugin_kit_android_background_sync() -> $crate::BackgroundSync {
            $config()
        }
    };
}

/// A [`LibName`] from a string literal, checked at compile time: an invalid
/// name is a build error, so the value cannot fail at run time.
///
/// ```
/// let name = tauri_plugin_kit_android::lib_name!("sickness_tracker_lib");
/// assert_eq!(name.as_str(), "sickness_tracker_lib");
/// ```
///
/// ```compile_fail
/// let _ = tauri_plugin_kit_android::lib_name!("libapp.so");
/// ```
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! lib_name {
    ($name:literal) => {
        const {
            match $crate::LibName::literal($name) {
                ::core::result::Result::Ok(name) => $crate::LibName::from_literal(name),
                ::core::result::Result::Err(_) => {
                    ::core::panic!(::core::concat!("invalid library name: ", $name))
                }
            }
        }
    };
}

/// A [`Schedule`] from two delays, checked at compile time: an out-of-range
/// delay is a build error, so the value cannot fail at run time.
///
/// ```
/// use std::time::Duration;
/// let schedule = tauri_plugin_kit_android::schedule!(
///     Duration::from_secs(10),
///     Duration::from_secs(30),
/// );
/// assert_eq!(schedule.initial_delay(), Duration::from_secs(10));
/// assert_eq!(schedule.backoff(), Duration::from_secs(30));
/// ```
///
/// ```compile_fail
/// use std::time::Duration;
/// let _ = tauri_plugin_kit_android::schedule!(Duration::from_secs(10), Duration::ZERO);
/// ```
#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! schedule {
    ($initial_delay:expr, $backoff:expr $(,)?) => {
        const {
            match $crate::Schedule::new($initial_delay, $backoff) {
                ::core::result::Result::Ok(schedule) => schedule,
                ::core::result::Result::Err($crate::ScheduleError::SubMillisecond) => {
                    ::core::panic!("schedule has a fractional millisecond")
                }
                ::core::result::Result::Err($crate::ScheduleError::Backoff) => {
                    ::core::panic!("backoff must be from 10 s to 5 h")
                }
                ::core::result::Result::Err($crate::ScheduleError::InitialDelay) => {
                    ::core::panic!("initial delay is too long for WorkManager")
                }
            }
        }
    };
}

#[cfg(test)]
mod tests;
