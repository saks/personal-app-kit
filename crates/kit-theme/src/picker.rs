//! `AppearancePicker`: the shared Settings → Appearance controls (#3).
//!
//! The kit renders the two fieldsets (Theme cards + Mode join) and owns the
//! behaviour; the app owns everything around them (section, heading, hint).
//! State lives in [`ThemeProvider`](crate::ThemeProvider): the picker reads
//! [`use_theme`] and picks through [`ThemeCtx::choose`](crate::ThemeCtx::choose).
//! It stores nothing and never calls [`ThemeApi`](crate::ThemeApi).

mod swatch;
mod view;

use yew::prelude::*;

use crate::prefs::{Shade, Theme, ThemePrefs};
use crate::provider::use_theme;
use crate::state::Pick;

/// Props for [`AppearancePicker`]. Labels for each option come from
/// [`ThemeFamily`](crate::ThemeFamily) and [`ThemeMode`](crate::ThemeMode).
#[derive(Properties, PartialEq, Clone)]
pub struct AppearancePickerProps {
    /// Legend above the family cards.
    #[prop_or(AttrValue::Static("Theme"))]
    pub theme_legend: AttrValue,
    /// Legend above the mode buttons.
    #[prop_or(AttrValue::Static("Mode"))]
    pub mode_legend: AttrValue,
    /// Body of each family card's preview. The kit wraps it in the
    /// `data-theme` / `aria-hidden` / `pointer-events-none` swatch frame, so the
    /// app draws content only. `None` = the kit's generic swatch.
    #[prop_or_default]
    pub preview: Option<Callback<Theme, Html>>,
    /// Extra classes on the picker's root `<div>` (spacing in the host page).
    #[prop_or_default]
    pub class: Classes,
}

/// What the picker shows: the prefs in effect (`None` while booting) and the
/// OS shade that `System` resolves against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PickerState {
    pub(crate) prefs: Option<ThemePrefs>,
    pub(crate) system: Shade,
}

impl PickerState {
    /// The shade the previews use: the picked mode resolved against the OS,
    /// or the OS shade while booting.
    pub(crate) fn shade(self) -> Shade {
        self.prefs
            .map_or(self.system, |prefs| prefs.shade(self.system))
    }
}

/// Theme family cards (with a live swatch preview) and a System / Light /
/// Dark mode join. A pick applies at once; the provider saves it.
///
/// - **Needs a [`ThemeProvider`](crate::ThemeProvider)** above it: outside
///   one it renders nothing.
/// - **One per page:** ids (`theme-family-*`, `theme-mode-*`) and radio names
///   (`theme-family`, `theme-mode`) are fixed.
/// - **The app owns the heading and hint** around it; the kit renders only
///   the two `fieldset`s, inside a `div` with `data-testid="appearance-picker"`.
/// - **While booting** (the app's store hasn't answered) both fieldsets are
///   disabled, nothing is checked and the root has `aria-busy="true"`.
#[function_component(AppearancePicker)]
pub fn appearance_picker(props: &AppearancePickerProps) -> Html {
    let Some(theme) = use_theme() else {
        return Html::default();
    };
    let state = PickerState {
        prefs: theme.source.prefs(),
        system: theme.system,
    };
    let on_pick = Callback::from(move |pick: Pick| theme.choose(pick));
    view::render(state, props, on_pick)
}

#[cfg(test)]
mod tests;
