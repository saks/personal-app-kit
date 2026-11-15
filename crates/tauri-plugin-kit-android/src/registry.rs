//! How the worker finds the app's `BackgroundSync` in a cold process.
//!
//! `background_sync!` defines one Rust function with a fixed symbol name
//! in the app crate; the worker declares and calls it.
//!
//! What a forgotten macro does, as actually confirmed:
//! - Linux ELF (checked with a throwaway `cdylib`): the build **succeeds**.
//!   A shared library may keep undefined symbols, so `nm -D` lists the
//!   symbol as `U`, and loading the library fails with `dlopen`'s
//!   "undefined symbol: __tauri_plugin_kit_android_background_sync".
//! - Android: not confirmed on a device. Rust's `aarch64-linux-android`
//!   target adds no `--no-undefined`, so expect the same: the build passes
//!   unless the app's linker flags add `-Wl,--no-undefined`, and
//!   `System.loadLibrary` throws `UnsatisfiedLinkError` ("cannot locate
//!   symbol"). Because the exported `runSync` JNI function references the
//!   symbol, the app's own library fails to load too, so the app fails at
//!   start rather than silently skipping background sync. `SyncWorker` fails
//!   the job (no retry) on that error.

use crate::config::BackgroundSync;

unsafe extern "Rust" {
    /// Defined by `background_sync!` in the app (in this crate's tests).
    safe fn __tauri_plugin_kit_android_background_sync() -> BackgroundSync;
}

/// The app's registered job.
pub(crate) fn registered() -> BackgroundSync {
    __tauri_plugin_kit_android_background_sync()
}
