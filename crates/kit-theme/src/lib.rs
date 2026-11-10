//! The shared theme set for both apps (sickness_tracker#93, from ST #45).
//!
//! - Default features: host-safe types. [`ThemePrefs`] (family + mode, with
//!   field-wise fallback), the [`Theme`] names, the [`MirrorKeys`] derived
//!   from the app's `StoragePrefix`, and the provider state machine.
//! - Feature `yew`: `ThemeProvider`, `use_theme`, `data-theme` apply, the
//!   `localStorage` mirror and Android `SystemBars`. The app injects
//!   persistence through [`ThemeApi`]. `AppearancePicker` renders the
//!   Settings → Appearance controls (theme cards with a live preview + mode
//!   join) on top of `use_theme`; the app keeps the heading and hint.
//! - Assets, shipped through the app's `scripts/kit-css.sh`
//!   (`docs/tailwind-across-crates.md`): `css/themes.css`,
//!   `css/appearance-picker.css` and `js/theme-boot.js`.

mod mirror;
mod prefs;
mod state;

pub use mirror::MirrorKeys;
pub use prefs::{Shade, Theme, ThemeFamily, ThemeMode, ThemePrefs};
pub use state::{Pick, Source, ThemeAction};

#[cfg(feature = "yew")]
mod provider;

#[cfg(feature = "yew")]
pub use provider::{
    ThemeApi, ThemeApiError, ThemeBackend, ThemeCtx, ThemeFuture, ThemeProvider,
    ThemeProviderProps, use_theme,
};

#[cfg(feature = "yew")]
mod picker;

#[cfg(feature = "yew")]
pub use picker::{AppearancePicker, AppearancePickerProps};

/// The boot script, for apps that check it in their own tests.
pub const BOOT_JS: &str = include_str!("../js/theme-boot.js");

/// The theme CSS, for apps that check it in their own tests.
pub const THEMES_CSS: &str = include_str!("../css/themes.css");

/// The `AppearancePicker` card CSS (`.theme-option`), for apps that check it
/// in their own tests.
pub const PICKER_CSS: &str = include_str!("../css/appearance-picker.css");

#[cfg(test)]
mod tests;
