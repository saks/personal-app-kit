use super::*;
use serde_json::json;

fn prefs(family: ThemeFamily, mode: ThemeMode) -> ThemePrefs {
    ThemePrefs { family, mode }
}

fn parse(value: serde_json::Value) -> ThemePrefs {
    serde_json::from_value(value).expect("theme prefs never fail on an object")
}

#[test]
fn default_is_default_system_and_leaves_data_theme_off() {
    let default = ThemePrefs::default();
    assert_eq!(default, prefs(ThemeFamily::Default, ThemeMode::System));
    assert_eq!(default.data_theme(Shade::Light), None);
    assert_eq!(default.data_theme(Shade::Dark), None);
}

#[test]
fn every_other_pair_names_a_theme() {
    use Shade::{Dark as SD, Light as SL};
    use ThemeFamily::{Default as D, Prisma as P};
    use ThemeMode::{Dark, Light, System};
    for (family, mode, system, theme) in [
        (D, Light, SD, Theme::AppLight),
        (D, Light, SL, Theme::AppLight),
        (D, Dark, SL, Theme::AppDark),
        (D, Dark, SD, Theme::AppDark),
        (P, System, SL, Theme::PrismaLight),
        (P, System, SD, Theme::PrismaDark),
        (P, Light, SD, Theme::PrismaLight),
        (P, Dark, SL, Theme::PrismaDark),
    ] {
        assert_eq!(
            prefs(family, mode).data_theme(system),
            Some(theme),
            "{family:?} {mode:?} system={system:?}"
        );
    }
}

#[test]
fn theme_names_are_the_daisyui_names() {
    assert_eq!(
        Theme::ALL.map(Theme::name),
        ["appLight", "appDark", "prismaLight", "prismaDark"]
    );
}

#[test]
fn serde_round_trips_with_the_value_names() {
    for family in ThemeFamily::ALL {
        for mode in ThemeMode::ALL {
            let p = prefs(family, mode);
            let value = serde_json::to_value(p).unwrap();
            assert_eq!(
                value,
                json!({ "family": family.value(), "mode": mode.value() })
            );
            assert_eq!(parse(value), p);
        }
    }
}

#[test]
fn unknown_wrong_type_or_missing_fields_fall_back_one_by_one() {
    assert_eq!(
        parse(json!({ "family": "neon", "mode": 7 })),
        ThemePrefs::default()
    );
    assert_eq!(
        parse(json!({ "family": "prisma", "mode": "dim" })),
        prefs(ThemeFamily::Prisma, ThemeMode::System)
    );
    assert_eq!(
        parse(json!({ "family": [1], "mode": "dark" })),
        prefs(ThemeFamily::Default, ThemeMode::Dark)
    );
    assert_eq!(
        parse(json!({ "family": null, "mode": { "x": 1 } })),
        ThemePrefs::default()
    );
    assert_eq!(
        parse(json!({ "mode": "light", "extra": true })),
        prefs(ThemeFamily::Default, ThemeMode::Light)
    );
    assert_eq!(parse(json!({})), ThemePrefs::default());
    let mode: ThemeMode = serde_json::from_value(json!("dim")).unwrap();
    assert_eq!(mode, ThemeMode::System);
}
