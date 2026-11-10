use futures::executor::block_on;
use yew::LocalServerRenderer;
use yew::prelude::*;

use super::{AppearancePickerProps, PickerState, view};
use crate::{Shade, Theme, ThemeFamily, ThemeMode, ThemePrefs};

#[derive(Properties, PartialEq)]
struct HarnessProps {
    state: PickerState,
    picker: AppearancePickerProps,
}

/// `view::render` with a no-op pick, so tests need no provider.
#[function_component(Harness)]
fn harness(props: &HarnessProps) -> Html {
    view::render(props.state, &props.picker, Callback::noop())
}

fn defaults() -> AppearancePickerProps {
    AppearancePickerProps {
        theme_legend: AttrValue::Static("Theme"),
        mode_legend: AttrValue::Static("Mode"),
        preview: None,
        class: Classes::new(),
    }
}

fn render_with(state: PickerState, picker: AppearancePickerProps) -> String {
    let renderer = LocalServerRenderer::<Harness>::with_props(HarnessProps { state, picker })
        .hydratable(false);
    block_on(renderer.render())
}

fn render(state: PickerState) -> String {
    render_with(state, defaults())
}

fn loaded(family: ThemeFamily, mode: ThemeMode, system: Shade) -> PickerState {
    PickerState {
        prefs: Some(ThemePrefs { family, mode }),
        system,
    }
}

/// The opening tag that contains `needle`.
fn tag<'a>(html: &'a str, needle: &str) -> &'a str {
    let at = html
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} not in {html}"));
    let start = html[..=at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

/// The family card `<label>` for `family`, through its `</label>`.
fn card(html: &str, family: ThemeFamily) -> &str {
    let open = tag(
        html,
        &format!("data-testid=\"theme-family-{}\"", family.value()),
    );
    let start = html.find(open).unwrap();
    let end = start + html[start..].find("</label>").unwrap();
    &html[start..end]
}

fn family_radio(html: &str, family: ThemeFamily) -> &str {
    tag(html, &format!(" id=\"theme-family-{}\"", family.value()))
}

fn mode_radio(html: &str, mode: ThemeMode) -> &str {
    tag(
        html,
        &format!("data-testid=\"theme-mode-{}\"", mode.value()),
    )
}

fn is_checked(tag: &str) -> bool {
    tag.contains(" checked")
}

fn swatch_theme(card: &str) -> &str {
    let at = card.find("data-theme=\"").unwrap() + "data-theme=\"".len();
    let len = card[at..].find('"').unwrap();
    &card[at..at + len]
}

#[test]
fn booting_disables_both_fieldsets_checks_nothing_and_is_busy() {
    let html = render(PickerState {
        prefs: None,
        system: Shade::Dark,
    });
    let fieldsets: Vec<_> = html.match_indices("<fieldset").collect();
    assert_eq!(fieldsets.len(), 2);
    for (at, _) in fieldsets {
        assert!(tag(&html[at..], "<fieldset").contains("disabled"), "{html}");
    }
    assert!(!html.contains(" checked"), "{html}");
    assert!(tag(&html, "data-testid=\"appearance-picker\"").contains("aria-busy=\"true\""));
    assert_eq!(swatch_theme(card(&html, ThemeFamily::Default)), "appDark");
    assert_eq!(swatch_theme(card(&html, ThemeFamily::Prisma)), "prismaDark");
}

#[test]
fn loaded_is_enabled_and_not_busy() {
    let html = render(loaded(
        ThemeFamily::Default,
        ThemeMode::System,
        Shade::Light,
    ));
    assert!(!html.contains("disabled"), "{html}");
    assert!(!html.contains("aria-busy"), "{html}");
}

#[test]
fn exactly_the_picked_family_and_mode_are_checked_with_live_swatches() {
    for system in Shade::ALL {
        for picked_family in ThemeFamily::ALL {
            for picked_mode in ThemeMode::ALL {
                let state = loaded(picked_family, picked_mode, system);
                let html = render(state);
                let shade = ThemePrefs {
                    family: picked_family,
                    mode: picked_mode,
                }
                .shade(system);
                for family in ThemeFamily::ALL {
                    let on = family == picked_family;
                    let card = card(&html, family);
                    assert_eq!(is_checked(family_radio(&html, family)), on);
                    let label = tag(card, "<label");
                    assert_eq!(label.contains("border-primary"), on, "{label}");
                    assert_eq!(label.contains("border-base-content/10"), !on, "{label}");
                    assert_eq!(swatch_theme(card), family.theme(shade).name());
                }
                for mode in ThemeMode::ALL {
                    let on = mode == picked_mode;
                    let radio = mode_radio(&html, mode);
                    assert_eq!(is_checked(radio), on, "{radio}");
                    assert_eq!(radio.contains("btn-primary"), on, "{radio}");
                }
            }
        }
    }
}

#[test]
fn system_mode_follows_the_os_shade() {
    for (system, name) in [(Shade::Light, "appLight"), (Shade::Dark, "appDark")] {
        let html = render(loaded(ThemeFamily::Default, ThemeMode::System, system));
        assert_eq!(swatch_theme(card(&html, ThemeFamily::Default)), name);
    }
}

#[test]
fn ids_testids_and_radio_names_are_the_drop_in_contract() {
    let html = render(loaded(ThemeFamily::Prisma, ThemeMode::Dark, Shade::Light));
    for family in ["default", "prisma"] {
        let radio = tag(&html, &format!(" id=\"theme-family-{family}\""));
        assert!(radio.contains("name=\"theme-family\""), "{radio}");
        assert!(radio.contains("type=\"radio\""), "{radio}");
        let label = tag(&html, &format!("data-testid=\"theme-family-{family}\""));
        assert!(
            label.contains(&format!("for=\"theme-family-{family}\"")),
            "{label}"
        );
    }
    for mode in ["system", "light", "dark"] {
        let radio = tag(&html, &format!("data-testid=\"theme-mode-{mode}\""));
        assert!(radio.contains("name=\"theme-mode\""), "{radio}");
        assert!(radio.contains("type=\"radio\""), "{radio}");
    }
    assert!(html.starts_with("<div"), "{html}");
    assert!(tag(&html, "<div").contains("data-testid=\"appearance-picker\""));
}

#[test]
fn legends_default_override_and_class_lands_on_the_root() {
    let state = loaded(ThemeFamily::Default, ThemeMode::Light, Shade::Light);
    let html = render(state);
    assert!(html.contains(">Theme</legend>"), "{html}");
    assert!(html.contains(">Mode</legend>"), "{html}");

    let html = render_with(
        state,
        AppearancePickerProps {
            theme_legend: "Colours".into(),
            mode_legend: "Brightness".into(),
            class: classes!("mt-4"),
            ..defaults()
        },
    );
    assert!(html.contains(">Colours</legend>"), "{html}");
    assert!(html.contains(">Brightness</legend>"), "{html}");
    let root = tag(&html, "data-testid=\"appearance-picker\"");
    assert!(root.contains("mt-4"), "{root}");
}

#[test]
fn a_custom_preview_replaces_only_the_body_inside_the_frame() {
    let preview = Callback::from(|theme: Theme| {
        html! { <span class="app-preview">{ format!("preview-{}", theme.name()) }</span> }
    });
    let html = render_with(
        loaded(ThemeFamily::Default, ThemeMode::Dark, Shade::Light),
        AppearancePickerProps {
            preview: Some(preview),
            ..defaults()
        },
    );
    for family in ThemeFamily::ALL {
        let card = card(&html, family);
        let frame = tag(card, "data-theme=");
        assert!(frame.contains("aria-hidden=\"true\""), "{frame}");
        assert!(frame.contains("pointer-events-none"), "{frame}");
        let name = family.theme(Shade::Dark).name();
        let body = &card[card.find(frame).unwrap() + frame.len()..];
        assert!(body.contains(&format!("preview-{name}")), "{card}");
        assert!(!body.contains("bg-success"), "default body leaked: {card}");
    }
}

#[test]
fn the_default_preview_is_generic() {
    let html = render(loaded(ThemeFamily::Prisma, ThemeMode::Light, Shade::Dark));
    for family in ThemeFamily::ALL {
        let card = card(&html, family);
        let frame = tag(card, "data-theme=");
        assert!(frame.contains("aria-hidden=\"true\""), "{frame}");
        let start = card.find(frame).unwrap() + frame.len();
        let body = &card[start..start + card[start..].find("<input").unwrap()];
        assert!(body.contains("bg-primary"), "{body}");
        // Text nodes only: no digits, units or app words.
        let text: String = body
            .split('<')
            .filter_map(|part| part.split_once('>').map(|(_, text)| text))
            .collect();
        assert!(text.trim().is_empty(), "app text in default swatch: {text}");
        assert!(!body.contains('°'), "{body}");
    }
}

#[test]
fn labels_and_blurbs_come_from_the_enums() {
    let html = render(loaded(
        ThemeFamily::Default,
        ThemeMode::System,
        Shade::Light,
    ));
    for family in ThemeFamily::ALL {
        let card = card(&html, family);
        assert!(
            card.contains(&format!(">{}</span>", family.label())),
            "{card}"
        );
        assert!(
            card.contains(&format!(">{}</span>", family.blurb())),
            "{card}"
        );
    }
    for mode in ThemeMode::ALL {
        let radio = mode_radio(&html, mode);
        assert!(
            radio.contains(&format!("aria-label=\"{}\"", mode.label())),
            "{radio}"
        );
    }
}
