//! The shared Android WorkManager background-sync job (sickness_tracker#96).
//!
//! The Android library under `android/` carries `SyncWorker`
//! (`com.saksmlz.kit.android`), so the JNI symbol of `runSync` is fixed by
//! this crate and no app writes a `Java_com_saksmlz_…` function. The app's
//! native library name, log tag and schedule travel in the WorkManager input
//! `Data`.
//!
//! - The app registers its job once, with [`background_sync!`]. That links
//!   a function the worker calls; it is not a runtime `init`, because in a
//!   cold process WorkManager starts the worker before any app code runs.
//! - [`enqueue`] (Android, and host tests): one network-constrained job,
//!   `REPLACE`, so at most one is queued.
//! - In a cold process the worker needs no Tauri runtime: it loads the app's
//!   library and calls [`BackgroundSync::run`] with the data dir. A panic
//!   becomes [`WorkerResult::Retry`].
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
mod registry;
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
mod worker;

#[cfg(not(target_arch = "wasm32"))]
pub use config::{BackgroundSync, LibName, LibNameError, Schedule};
#[cfg(any(target_os = "android", all(test, not(target_arch = "wasm32"))))]
pub use jni_glue::enqueue;
#[cfg(not(target_arch = "wasm32"))]
pub use result::WorkerResult;

/// Registers the app's background job. Call it once, at the top level of the
/// app's library crate, with a `fn() -> BackgroundSync`:
///
/// ```ignore
/// fn background_sync() -> tauri_plugin_kit_android::BackgroundSync { /* … */ }
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

#[cfg(test)]
mod tests;
