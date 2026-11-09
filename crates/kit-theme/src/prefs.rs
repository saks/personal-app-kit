//! Theme preference: family × mode, device-local, never synced.
//!
//! `data-theme` on `<html>` for each pair:
//!
//! | family \ mode | System                         | Light         | Dark         |
//! |---------------|--------------------------------|---------------|--------------|
//! | Default       | *off* (daisyUI `prefersdark`)  | `appLight`    | `appDark`    |
//! | Prisma        | `prismaLight` / `prismaDark`   | `prismaLight` | `prismaDark` |
//!
//! Default + System leaves the attribute off, so `appLight` / `appDark`
//! follow `prefers-color-scheme` in CSS alone.
//!
//! Stored values that are missing, unknown or of the wrong type fall back to
//! Default / System field by field; reading them never fails.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeFamily {
    Prisma,
    /// Last, so `serde(other)` maps unknown names here.
    #[default]
    #[serde(other)]
    Default,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    Dark,
    /// Last, so `serde(other)` maps unknown names here.
    #[default]
    #[serde(other)]
    System,
}

/// Light or dark, once `System` is resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shade {
    Light,
    Dark,
}

/// A daisyUI theme in `css/themes.css`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    AppLight,
    AppDark,
    PrismaLight,
    PrismaDark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemePrefs {
    #[serde(deserialize_with = "or_default")]
    pub family: ThemeFamily,
    #[serde(deserialize_with = "or_default")]
    pub mode: ThemeMode,
}

/// A field of the wrong type (`"mode": 7`) reads as its default instead of
/// failing the whole value.
fn or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OrGarbage<T> {
        Known(T),
        Garbage(de::IgnoredAny),
    }
    Ok(match OrGarbage::<T>::deserialize(deserializer)? {
        OrGarbage::Known(value) => value,
        OrGarbage::Garbage(_) => T::default(),
    })
}

impl ThemeFamily {
    /// Display order in a picker.
    pub const ALL: [ThemeFamily; 2] = [ThemeFamily::Default, ThemeFamily::Prisma];

    /// The serialized name, also the `localStorage` mirror value.
    pub const fn value(self) -> &'static str {
        match self {
            ThemeFamily::Default => "default",
            ThemeFamily::Prisma => "prisma",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ThemeFamily::Default => "Default",
            ThemeFamily::Prisma => "Prisma",
        }
    }

    pub const fn blurb(self) -> &'static str {
        match self {
            ThemeFamily::Default => "Calm slate",
            ThemeFamily::Prisma => "Vivid colour",
        }
    }

    pub const fn theme(self, shade: Shade) -> Theme {
        match (self, shade) {
            (ThemeFamily::Default, Shade::Light) => Theme::AppLight,
            (ThemeFamily::Default, Shade::Dark) => Theme::AppDark,
            (ThemeFamily::Prisma, Shade::Light) => Theme::PrismaLight,
            (ThemeFamily::Prisma, Shade::Dark) => Theme::PrismaDark,
        }
    }
}

impl ThemeMode {
    /// Display order in a picker.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    /// The serialized name, also the `localStorage` mirror value.
    pub const fn value(self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }
}

impl Shade {
    pub const ALL: [Shade; 2] = [Shade::Light, Shade::Dark];

    pub const fn is_dark(self) -> bool {
        matches!(self, Shade::Dark)
    }
}

impl Theme {
    pub const ALL: [Theme; 4] = [
        Theme::AppLight,
        Theme::AppDark,
        Theme::PrismaLight,
        Theme::PrismaDark,
    ];

    /// The `data-theme` value and the daisyUI theme `name` in `css/themes.css`.
    pub const fn name(self) -> &'static str {
        match self {
            Theme::AppLight => "appLight",
            Theme::AppDark => "appDark",
            Theme::PrismaLight => "prismaLight",
            Theme::PrismaDark => "prismaDark",
        }
    }
}

impl ThemePrefs {
    /// Light or dark once System is resolved against the OS shade.
    pub const fn shade(self, system: Shade) -> Shade {
        match self.mode {
            ThemeMode::System => system,
            ThemeMode::Light => Shade::Light,
            ThemeMode::Dark => Shade::Dark,
        }
    }

    /// The `data-theme` value; `None` leaves the attribute off (Default +
    /// System).
    pub fn data_theme(self, system: Shade) -> Option<Theme> {
        (self != ThemePrefs::default()).then(|| self.family.theme(self.shade(system)))
    }
}

#[cfg(test)]
mod tests;
