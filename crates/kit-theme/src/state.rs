//! The provider's state machine, host-safe so it is tested without a browser.
//!
//! [`Source`] says where the prefs in effect came from. Until the app's
//! `ThemeApi` answers (`Boot`), the boot script's `data-theme` stands and a
//! picker stays disabled.

use crate::prefs::{Shade, ThemeFamily, ThemeMode, ThemePrefs};

/// Where the preference in effect came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The app's store hasn't answered; `theme-boot.js` applied the mirror.
    Boot,
    /// The stored value.
    Saved(ThemePrefs),
    /// The store could not be read: Default / System for this session (so
    /// CSS follows the OS), mirror left as it was.
    Fallback,
    /// Picked this session.
    Chosen(ThemePrefs),
}

/// One tap in a picker: changes one half and keeps the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Family(ThemeFamily),
    Mode(ThemeMode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeAction {
    Loaded(ThemePrefs),
    LoadFailed,
    Choose(Pick),
}

impl Pick {
    pub const fn apply(self, prefs: ThemePrefs) -> ThemePrefs {
        match self {
            Pick::Family(family) => ThemePrefs { family, ..prefs },
            Pick::Mode(mode) => ThemePrefs { mode, ..prefs },
        }
    }
}

impl Source {
    /// The prefs in effect; `None` while booting.
    pub fn prefs(self) -> Option<ThemePrefs> {
        match self {
            Source::Boot => None,
            Source::Fallback => Some(ThemePrefs::default()),
            Source::Saved(prefs) | Source::Chosen(prefs) => Some(prefs),
        }
    }

    /// The resolved shade for the Android bar icons; `None` while booting
    /// (the bars keep what `enableEdgeToEdge` picked).
    pub fn bars_shade(self, system: Shade) -> Option<Shade> {
        self.prefs().map(|prefs| prefs.shade(system))
    }

    /// What the mirror should hold: loaded or saved prefs only.
    pub fn mirrored(self) -> Option<ThemePrefs> {
        match self {
            Source::Saved(prefs) | Source::Chosen(prefs) => Some(prefs),
            Source::Boot | Source::Fallback => None,
        }
    }

    pub fn next(self, action: ThemeAction) -> Source {
        match (self, action) {
            // A choice made meanwhile wins: a late reply must not undo it.
            (Source::Chosen(_), ThemeAction::Loaded(_) | ThemeAction::LoadFailed) => self,
            (_, ThemeAction::Loaded(saved)) => Source::Saved(saved),
            (Source::Boot, ThemeAction::LoadFailed) => Source::Fallback,
            (_, ThemeAction::LoadFailed) => self,
            // Nothing to pick against while booting (a picker is disabled).
            (Source::Boot, ThemeAction::Choose(_)) => self,
            (_, ThemeAction::Choose(pick)) => {
                Source::Chosen(pick.apply(self.prefs().unwrap_or_default()))
            }
        }
    }
}

/// One save in flight at a time; a pick made meanwhile replaces any queued
/// one, so the store ends with the latest choice.
#[cfg(any(feature = "yew", test))]
#[derive(Debug, Default)]
pub(crate) struct SaveQueue {
    in_flight: bool,
    queued: Option<ThemePrefs>,
}

#[cfg(any(feature = "yew", test))]
impl SaveQueue {
    /// The prefs to send now, or `None` if a save is in flight (then queued).
    pub(crate) fn push(&mut self, prefs: ThemePrefs) -> Option<ThemePrefs> {
        if self.in_flight {
            self.queued = Some(prefs);
            None
        } else {
            self.in_flight = true;
            Some(prefs)
        }
    }

    /// The save in flight finished: the queued prefs to send next, if any.
    pub(crate) fn done(&mut self) -> Option<ThemePrefs> {
        let next = self.queued.take();
        self.in_flight = next.is_some();
        next
    }
}

#[cfg(test)]
mod tests;
