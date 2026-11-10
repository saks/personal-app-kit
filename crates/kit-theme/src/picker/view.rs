//! The picker's markup as a pure function of [`PickerState`] and props, so
//! it renders in tests without a provider. Markup, classes, ids and radio
//! names match sickness_tracker's Settings → Appearance, so adopting it is a
//! drop-in.

use yew::prelude::*;

use super::{AppearancePickerProps, PickerState, swatch};
use crate::prefs::{ThemeFamily, ThemeMode};
use crate::state::Pick;

const CARD_CLASS: &str = "theme-option card cursor-pointer border-2 bg-base-200 p-2 has-focus-visible:outline-2 has-focus-visible:outline-offset-2 has-focus-visible:outline-base-content";
const MODE_CLASS: &str = "btn join-item h-12 min-h-12 flex-1 px-0";

fn card_class(checked: bool) -> Classes {
    classes!(
        CARD_CLASS,
        if checked {
            "border-primary"
        } else {
            "border-base-content/10"
        }
    )
}

fn mode_class(checked: bool) -> Classes {
    classes!(
        MODE_CLASS,
        if checked {
            "btn-primary"
        } else {
            "border-base-content/10 bg-base-200"
        }
    )
}

fn family_card(
    state: PickerState,
    props: &AppearancePickerProps,
    on_pick: &Callback<Pick>,
    family: ThemeFamily,
) -> Html {
    let checked = state.prefs.is_some_and(|prefs| prefs.family == family);
    let onchange = on_pick.reform(move |_: Event| Pick::Family(family));
    let theme = family.theme(state.shade());
    let body = props
        .preview
        .as_ref()
        .map_or_else(swatch::default_body, |preview| preview.emit(theme));
    let id = format!("theme-family-{}", family.value());
    html! {
        <label class={card_class(checked)} for={id.clone()} data-testid={id.clone()}>
            { swatch::frame(theme, body) }
            <span class="flex min-h-11 items-center gap-2 px-1 pt-2">
                <input
                    type="radio"
                    class="radio radio-primary radio-sm"
                    name="theme-family"
                    id={id}
                    value={family.value()}
                    {checked}
                    {onchange}
                />
                <span class="leading-tight">
                    <span class="block font-medium">{ family.label() }</span>
                    <span class="block text-xs opacity-70">{ family.blurb() }</span>
                </span>
            </span>
        </label>
    }
}

fn mode_button(state: PickerState, on_pick: &Callback<Pick>, mode: ThemeMode) -> Html {
    let checked = state.prefs.is_some_and(|prefs| prefs.mode == mode);
    let onchange = on_pick.reform(move |_: Event| Pick::Mode(mode));
    html! {
        <input
            type="radio"
            class={mode_class(checked)}
            name="theme-mode"
            value={mode.value()}
            aria-label={mode.label()}
            data-testid={format!("theme-mode-{}", mode.value())}
            {checked}
            {onchange}
        />
    }
}

pub(super) fn render(
    state: PickerState,
    props: &AppearancePickerProps,
    on_pick: Callback<Pick>,
) -> Html {
    // `None` until the app's store answers: nothing checked, radios
    // disabled, so a pick can't be made against a guess.
    let booting = state.prefs.is_none();
    let families = ThemeFamily::ALL
        .iter()
        .map(|&family| family_card(state, props, &on_pick, family));
    let modes = ThemeMode::ALL
        .iter()
        .map(|&mode| mode_button(state, &on_pick, mode));
    html! {
        <div
            class={classes!("space-y-3", props.class.clone())}
            data-testid="appearance-picker"
            aria-busy={booting.then_some("true")}
        >
            <fieldset class="space-y-2" disabled={booting}>
                <legend class="mb-2 text-sm font-medium">{ props.theme_legend.clone() }</legend>
                <div class="grid grid-cols-2 gap-3">{ for families }</div>
            </fieldset>
            <fieldset class="space-y-2" disabled={booting}>
                <legend class="mb-2 text-sm font-medium">{ props.mode_legend.clone() }</legend>
                <div class="join w-full">{ for modes }</div>
            </fieldset>
        </div>
    }
}
