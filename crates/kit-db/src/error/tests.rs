use super::Error;

#[test]
fn typed_and_text_busy_errors_are_busy() {
    assert!(Error::from(turso::Error::Busy("x".into())).is_busy());
    assert!(Error::from(turso::Error::BusySnapshot("x".into())).is_busy());
    assert!(Error::from(turso::Error::Error("Database is busy".into())).is_busy());
    assert!(
        Error::from(turso::Error::Error(
            "a sync operation is in progress".into()
        ))
        .is_busy()
    );
    assert!(!Error::from(turso::Error::Error("no such table: t".into())).is_busy());
    assert!(!Error::NotANumber.is_busy());
}

#[test]
fn messages_match_the_apps() {
    assert_eq!(Error::NotANumber.to_string(), "value is not a number");
    assert_eq!(Error::NotAnInteger.to_string(), "value is not an integer");
    assert_eq!(
        Error::IntegerOverflow.to_string(),
        "u64 value does not fit in SQLite integer"
    );
    assert_eq!(
        Error::UnsupportedEnum.to_string(),
        "unsupported sea-query enum value"
    );
    assert_eq!(
        Error::InvalidClock {
            var: "MY_APP_CLOCK".into()
        }
        .to_string(),
        "MY_APP_CLOCK must be YYYY-MM-DD"
    );
}
