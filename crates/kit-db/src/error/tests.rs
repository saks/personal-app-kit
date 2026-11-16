use super::{Error, Retry};

#[test]
fn retry_level_follows_the_turso_variant() {
    let locked = Error::from(turso::Error::Busy("database is locked".into()));
    assert_eq!(locked.retry(), Some(Retry::Statement));
    assert!(locked.is_busy());

    let in_progress = Error::from(turso::Error::Busy(
        "cannot commit transaction - SQL statements in progress".into(),
    ));
    assert_eq!(in_progress.retry(), None);
    assert!(!in_progress.is_busy());

    let snapshot = Error::from(turso::Error::BusySnapshot(
        "database snapshot is stale, rollback and retry the transaction".into(),
    ));
    assert_eq!(snapshot.retry(), Some(Retry::Transaction));
    assert!(snapshot.is_busy());

    let text = Error::from(turso::Error::Error("Database is busy".into()));
    assert_eq!(text.retry(), Some(Retry::Statement));
    assert!(text.is_busy());

    let tape = Error::from(turso::Error::Error("database is busy".into()));
    assert_eq!(tape.retry(), Some(Retry::Statement));
    assert!(tape.is_busy());

    let sync_phrase = Error::from(turso::Error::Error(
        "a sync operation is in progress".into(),
    ));
    assert_eq!(sync_phrase.retry(), None);
    assert!(!sync_phrase.is_busy());

    let missing = Error::from(turso::Error::Error("no such table: t".into()));
    assert_eq!(missing.retry(), None);
    assert!(!missing.is_busy());

    assert_eq!(Error::NotANumber.retry(), None);
    assert!(!Error::NotANumber.is_busy());
    assert!(!Error::NestedTransaction.is_busy());
    assert!(!Error::TransactionControl.is_busy());
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
    assert_eq!(
        Error::NestedTransaction.to_string(),
        "cannot start a transaction within a transaction"
    );
    assert_eq!(
        Error::TransactionControl.to_string(),
        "transaction control statements are not allowed inside a transaction"
    );
}
