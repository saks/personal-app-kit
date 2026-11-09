//! How the worker finds the app's [`BackgroundSync`] in a cold process.
//!
//! [`background_sync!`] defines one Rust function with a fixed symbol name
//! in the app crate; the worker declares and calls it. Forgetting the macro
//! is a link error in the app's build, not a silent no-op on the phone.

use crate::config::BackgroundSync;

unsafe extern "Rust" {
    /// Defined by [`background_sync!`] in the app (in this crate's tests).
    safe fn __tauri_plugin_kit_android_background_sync() -> BackgroundSync;
}

/// The app's registered job.
pub(crate) fn registered() -> BackgroundSync {
    __tauri_plugin_kit_android_background_sync()
}
