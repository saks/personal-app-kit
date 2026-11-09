//! [`UtcMillis`]: the one form timestamps are stored in.
//!
//! RFC 3339, UTC, milliseconds, `Z` (`2026-10-18T12:00:00.123Z`). Stored
//! values compare as text in time order only if every write and every query
//! bound uses it, so parse once at the edge and pass the type around.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, SecondsFormat, SubsecRound, Utc};

use crate::error::Error;

/// A UTC instant at millisecond precision. `Display` is the stored form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcMillis(DateTime<Utc>);

/// Offset-less inputs, read as UTC.
const NAIVE_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M",
];

impl UtcMillis {
    pub fn now() -> Self {
        Self::from(Utc::now())
    }

    pub fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }

    pub fn timestamp_millis(self) -> i64 {
        self.0.timestamp_millis()
    }

    /// The calendar day of this instant in the device's local time zone.
    pub fn local_date(self) -> NaiveDate {
        self.0.with_timezone(&Local).date_naive()
    }
}

/// Truncates to milliseconds, like the stored form.
impl From<DateTime<Utc>> for UtcMillis {
    fn from(at: DateTime<Utc>) -> Self {
        Self(at.trunc_subsecs(3))
    }
}

/// RFC 3339 with any offset, or an offset-less ISO-8601 datetime read as
/// UTC. Surrounding whitespace is ignored.
impl FromStr for UtcMillis {
    type Err = Error;

    fn from_str(raw: &str) -> Result<Self, Error> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(Error::EmptyTimestamp);
        }
        if let Ok(at) = DateTime::parse_from_rfc3339(raw) {
            return Ok(Self::from(at.with_timezone(&Utc)));
        }
        NAIVE_FORMATS
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(raw, format).ok())
            .map(|naive| Self::from(naive.and_utc()))
            .ok_or(Error::InvalidTimestamp)
    }
}

impl fmt::Display for UtcMillis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

/// The local calendar day of a stored RFC 3339 value; `None` if it isn't one.
pub fn local_date_of(rfc3339: &str) -> Option<NaiveDate> {
    let at = DateTime::parse_from_rfc3339(rfc3339).ok()?;
    Some(at.with_timezone(&Local).date_naive())
}

#[cfg(test)]
mod tests;
