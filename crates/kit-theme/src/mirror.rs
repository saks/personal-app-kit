//! The `localStorage` mirror keys, derived from the app's `StoragePrefix`.
//! Only `js/theme-boot.js` reads the mirror, before the first paint.

use kit_core::StoragePrefix;

use crate::prefs::ThemePrefs;

/// `<prefix>.theme` and `<prefix>.mode`, as `js/theme-boot.js` builds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorKeys {
    family: String,
    mode: String,
}

impl MirrorKeys {
    /// Key names after the prefix. `js/theme-boot.js` uses the same.
    pub const FAMILY: &'static str = "theme";
    pub const MODE: &'static str = "mode";

    pub fn new(prefix: &StoragePrefix) -> Self {
        Self {
            family: prefix.key(Self::FAMILY),
            mode: prefix.key(Self::MODE),
        }
    }

    /// `(key, value)` pairs to write for `prefs`.
    pub fn entries(&self, prefs: ThemePrefs) -> [(&str, &'static str); 2] {
        [
            (self.family.as_str(), prefs.family.value()),
            (self.mode.as_str(), prefs.mode.value()),
        ]
    }
}
