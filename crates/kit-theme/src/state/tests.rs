use super::*;
use ThemeAction::{Choose, LoadFailed, Loaded};

fn prefs(family: ThemeFamily, mode: ThemeMode) -> ThemePrefs {
    ThemePrefs { family, mode }
}

fn run(start: Source, actions: &[ThemeAction]) -> Source {
    actions
        .iter()
        .fold(start, |source, &action| source.next(action))
}

#[test]
fn the_store_replaces_the_boot_state() {
    let saved = prefs(ThemeFamily::Prisma, ThemeMode::Dark);
    assert_eq!(Source::Boot.next(Loaded(saved)), Source::Saved(saved));
    assert_eq!(
        Source::Saved(ThemePrefs::default()).next(Loaded(saved)),
        Source::Saved(saved)
    );
    assert_eq!(Source::Boot.prefs(), None);
    assert_eq!(Source::Saved(saved).prefs(), Some(saved));
    assert_eq!(Source::Saved(saved).mirrored(), Some(saved));
}

#[test]
fn a_late_store_reply_keeps_the_users_choice() {
    let chosen = prefs(ThemeFamily::Prisma, ThemeMode::Light);
    let stale = prefs(ThemeFamily::Default, ThemeMode::System);
    assert_eq!(
        Source::Chosen(chosen).next(Loaded(stale)),
        Source::Chosen(chosen)
    );
    assert_eq!(
        Source::Chosen(chosen).next(LoadFailed),
        Source::Chosen(chosen)
    );
}

#[test]
fn a_pick_during_boot_is_ignored_and_after_loaded_keeps_the_family() {
    let prisma = prefs(ThemeFamily::Prisma, ThemeMode::System);
    let dark = Choose(Pick::Mode(ThemeMode::Dark));
    assert_eq!(Source::Boot.next(dark), Source::Boot);
    assert_eq!(
        run(Source::Boot, &[dark, Loaded(prisma)]),
        Source::Saved(prisma)
    );
    assert_eq!(
        run(Source::Boot, &[Loaded(prisma), dark]),
        Source::Chosen(prefs(ThemeFamily::Prisma, ThemeMode::Dark))
    );
    assert_eq!(
        run(
            Source::Boot,
            &[Loaded(prisma), Choose(Pick::Family(ThemeFamily::Default))]
        ),
        Source::Chosen(prefs(ThemeFamily::Default, ThemeMode::System))
    );
}

#[test]
fn a_failed_load_falls_back_to_default_system_and_enables_the_picker() {
    let fallback = Source::Boot.next(LoadFailed);
    assert_eq!(fallback, Source::Fallback);
    assert_eq!(fallback.prefs(), Some(ThemePrefs::default()));
    assert_eq!(ThemePrefs::default().data_theme(Shade::Dark), None);
    assert_eq!(fallback.mirrored(), None);
    assert_eq!(
        fallback.next(Choose(Pick::Mode(ThemeMode::Dark))),
        Source::Chosen(prefs(ThemeFamily::Default, ThemeMode::Dark))
    );
}

#[test]
fn bar_icons_follow_the_resolved_shade_once_loaded() {
    let light = prefs(ThemeFamily::Prisma, ThemeMode::Light);
    let dark = prefs(ThemeFamily::Default, ThemeMode::Dark);
    let system = prefs(ThemeFamily::Prisma, ThemeMode::System);
    assert_eq!(
        Source::Saved(light).bars_shade(Shade::Dark),
        Some(Shade::Light)
    );
    assert_eq!(
        Source::Chosen(dark).bars_shade(Shade::Light),
        Some(Shade::Dark)
    );
    assert_eq!(
        Source::Saved(system).bars_shade(Shade::Dark),
        Some(Shade::Dark)
    );
    assert_eq!(
        Source::Saved(system).bars_shade(Shade::Light),
        Some(Shade::Light)
    );
    assert_eq!(Source::Fallback.bars_shade(Shade::Dark), Some(Shade::Dark));
    assert_eq!(Source::Boot.bars_shade(Shade::Dark), None);
}

#[test]
fn the_latest_choice_is_saved_last() {
    let a = prefs(ThemeFamily::Prisma, ThemeMode::Light);
    let b = prefs(ThemeFamily::Prisma, ThemeMode::Dark);
    let c = prefs(ThemeFamily::Default, ThemeMode::Dark);
    let d = prefs(ThemeFamily::Default, ThemeMode::Light);
    let mut queue = SaveQueue::default();
    assert_eq!(queue.push(a), Some(a), "nothing in flight: send now");
    assert_eq!(queue.push(b), None, "a in flight: queued");
    assert_eq!(queue.push(c), None, "c replaces b");
    assert_eq!(queue.done(), Some(c), "a done: send the latest");
    assert_eq!(queue.done(), None, "c done: idle");
    assert_eq!(queue.push(d), Some(d));
}
