use chrono::{DateTime, NaiveDate, Utc};

use super::*;

fn parse(raw: &str) -> UtcMillis {
    raw.parse().unwrap()
}

#[test]
fn stored_form_is_utc_millis_with_z() {
    let at = DateTime::parse_from_rfc3339("2026-10-18T05:00:00.123456-07:00")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(UtcMillis::from(at).to_string(), "2026-10-18T12:00:00.123Z");
    assert_eq!(
        parse("2026-10-18T05:00:00.123-07:00").to_string(),
        "2026-10-18T12:00:00.123Z"
    );
    assert_eq!(
        parse("2026-10-18T12:00").to_string(),
        "2026-10-18T12:00:00.000Z"
    );
}

#[test]
fn naive_forms_read_as_utc() {
    for raw in [
        "2026-10-18T12:00:00.5",
        "2026-10-18T12:00:00",
        "2026-10-18 12:00:00",
        " 2026-10-18T12:00 ",
    ] {
        assert!(
            parse(raw).to_string().starts_with("2026-10-18T12:00:00."),
            "{raw}"
        );
    }
}

#[test]
fn blank_and_garbage_are_typed_errors() {
    assert!(matches!(
        "  ".parse::<UtcMillis>(),
        Err(Error::EmptyTimestamp)
    ));
    assert!(matches!(
        "yesterday".parse::<UtcMillis>(),
        Err(Error::InvalidTimestamp)
    ));
}

#[test]
fn round_trips_through_the_stored_form_and_orders_like_text() {
    let a = parse("2026-10-18T12:00:00.123Z");
    let b = parse("2026-10-18T12:00:00.124Z");
    assert_eq!(parse(&a.to_string()), a);
    assert!(a < b);
    assert!(a.to_string() < b.to_string());
    assert_eq!(a.timestamp_millis() + 1, b.timestamp_millis());
}

#[test]
fn now_is_already_truncated_to_millis() {
    let now = UtcMillis::now();
    assert_eq!(parse(&now.to_string()), now);
}

#[test]
fn local_date_of_matches_the_type_and_rejects_garbage() {
    let at = parse("2026-10-18T12:00:00.000Z");
    assert_eq!(local_date_of(&at.to_string()), Some(at.local_date()));
    assert_eq!(local_date_of("nope"), None);
    let noon = local_date_of("2026-06-15T12:00:00Z").unwrap();
    let day = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
    assert!((noon - day).num_days().abs() <= 1);
}
