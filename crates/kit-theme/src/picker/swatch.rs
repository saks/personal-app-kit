//! The family card preview: a frame in the theme's own tokens around a body
//! (the app's, or the kit's generic one).

use yew::prelude::*;

use crate::prefs::Theme;

/// The preview frame: `data-theme` scopes the body to `theme`; decorative
/// only, so hidden from screen readers and from clicks.
pub(super) fn frame(theme: Theme, body: Html) -> Html {
    html! {
        <div data-theme={theme.name()} class="theme-swatch pointer-events-none space-y-1.5 rounded-field bg-base-100 p-2 text-base-content" aria-hidden="true">
            { body }
        </div>
    }
}

/// The kit's generic body: a mini card row and a token strip (primary,
/// success, warning, error). No app text.
pub(super) fn default_body() -> Html {
    html! {
        <>
            <div class="rounded-field border border-base-content/10 bg-base-200 px-2 py-1.5">
                <div class="flex items-center gap-1">
                    <span class="size-2.5 rounded-full bg-primary"></span>
                    <span class="h-1.5 flex-1 rounded-full bg-base-content/20"></span>
                </div>
            </div>
            <div class="flex items-center gap-1">
                <span class="h-3 flex-1 rounded-field bg-primary"></span>
                <span class="size-3 rounded-full bg-success"></span>
                <span class="size-3 rounded-full bg-warning"></span>
                <span class="size-3 rounded-full bg-error"></span>
            </div>
        </>
    }
}
