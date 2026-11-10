use kit_core::StoragePrefix;

use super::{
    BOOT_JS, MirrorKeys, PICKER_CSS, Shade, THEMES_CSS, Theme, ThemeFamily, ThemeMode, ThemePrefs,
};

fn prefix() -> StoragePrefix {
    "my-app".parse().unwrap()
}

#[test]
fn mirror_keys_come_from_the_storage_prefix() {
    let keys = MirrorKeys::new(&prefix());
    let prefs = ThemePrefs {
        family: ThemeFamily::Prisma,
        mode: ThemeMode::Dark,
    };
    assert_eq!(
        keys.entries(prefs),
        [("my-app.theme", "prisma"), ("my-app.mode", "dark")]
    );
}

#[test]
fn mirror_values_are_the_serde_names_the_boot_script_reads() {
    let keys = MirrorKeys::new(&prefix());
    for family in ThemeFamily::ALL {
        for mode in ThemeMode::ALL {
            let p = ThemePrefs { family, mode };
            let json = serde_json::to_value(p).unwrap();
            let [(_, f), (_, m)] = keys.entries(p);
            assert_eq!(f, json["family"].as_str().unwrap());
            assert_eq!(m, json["mode"].as_str().unwrap());
        }
    }
}

/// `js/theme-boot.js` mirrors `MirrorKeys` and `ThemePrefs::data_theme`.
#[test]
fn the_boot_script_uses_the_same_keys_and_theme_names() {
    for needle in [
        "dataset.storagePrefix",
        &format!("prefix + \".{}\"", MirrorKeys::FAMILY),
        &format!("prefix + \".{}\"", MirrorKeys::MODE),
        "(prefers-color-scheme: dark)",
        "\"prisma\"",
        "\"light\"",
        "\"dark\"",
    ] {
        assert!(BOOT_JS.contains(needle), "{needle}");
    }
    for family in ThemeFamily::ALL {
        for shade in Shade::ALL {
            let name = family.theme(shade).name();
            assert!(BOOT_JS.contains(&format!("\"{name}\"")), "{name}");
        }
    }
}

#[test]
fn every_family_and_shade_has_a_theme_block_in_the_css() {
    for family in ThemeFamily::ALL {
        for shade in Shade::ALL {
            let name = family.theme(shade).name();
            assert!(
                THEMES_CSS.contains(&format!("name: \"{name}\";")),
                "{name} missing from css/themes.css"
            );
        }
    }
    assert_eq!(
        THEMES_CSS.matches("\n@plugin \"daisyui-theme\" {").count(),
        Theme::ALL.len(),
        "one placeholder theme block per Theme"
    );
}

#[test]
fn the_css_ships_the_component_accents_and_picker_cards_ship_apart() {
    for component in [".dock .dock-active", ".join-item.btn-primary"] {
        assert!(THEMES_CSS.contains(component), "{component}");
    }
    assert!(!THEMES_CSS.contains(".theme-option"));
    for rule in [".theme-option:has(:checked)", ".theme-option .radio"] {
        assert!(PICKER_CSS.contains(rule), "{rule}");
    }
}
