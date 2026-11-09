use super::*;

const ALL: [SyncTrigger; 9] = [
    SyncTrigger::Auto,
    SyncTrigger::Manual,
    SyncTrigger::Write,
    SyncTrigger::Flush,
    SyncTrigger::Retry,
    SyncTrigger::Online,
    SyncTrigger::Background,
    SyncTrigger::Exit,
    SyncTrigger::Settings,
];

#[test]
fn triggers_serialize_as_one_lowercase_word() {
    let names = ALL.map(|trigger| serde_json::to_string(&trigger).unwrap());
    assert_eq!(
        names,
        [
            "\"auto\"",
            "\"manual\"",
            "\"write\"",
            "\"flush\"",
            "\"retry\"",
            "\"online\"",
            "\"background\"",
            "\"exit\"",
            "\"settings\"",
        ]
    );
    for trigger in ALL {
        let json = serde_json::to_string(&trigger).unwrap();
        assert_eq!(serde_json::from_str::<SyncTrigger>(&json).unwrap(), trigger);
    }
}

#[test]
fn ported_from_st_trigger_serialize() {
    assert_eq!(
        serde_json::to_string(&SyncTrigger::Manual).unwrap(),
        "\"manual\""
    );
    assert_eq!(
        serde_json::to_string(&SyncTrigger::Background).unwrap(),
        "\"background\""
    );
    assert!(SyncTrigger::Background.from_host() && SyncTrigger::Background.automatic());
}

#[test]
fn only_manual_is_not_automatic() {
    for trigger in ALL {
        assert_eq!(
            trigger.automatic(),
            trigger != SyncTrigger::Manual,
            "{trigger:?}"
        );
    }
}

#[test]
fn host_triggers() {
    let host: Vec<_> = ALL.into_iter().filter(|t| t.from_host()).collect();
    assert_eq!(
        host,
        [
            SyncTrigger::Write,
            SyncTrigger::Flush,
            SyncTrigger::Retry,
            SyncTrigger::Background,
            SyncTrigger::Exit,
            SyncTrigger::Settings,
        ]
    );
}
