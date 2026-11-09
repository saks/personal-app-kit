use std::fmt;
use std::str::FromStr;

/// Which identity value failed to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityKind {
    EnvPrefix,
    LogTag,
    StoragePrefix,
}

impl fmt::Display for IdentityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EnvPrefix => "env prefix",
            Self::LogTag => "log tag",
            Self::StoragePrefix => "storage prefix",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseIdentityError {
    Empty(IdentityKind),
    TooLong {
        kind: IdentityKind,
        max: usize,
        len: usize,
    },
    InvalidChar {
        kind: IdentityKind,
        at: usize,
        ch: char,
    },
}

impl fmt::Display for ParseIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(kind) => write!(f, "{kind} is empty"),
            Self::TooLong { kind, max, len } => {
                write!(f, "{kind} is {len} characters, the limit is {max}")
            }
            Self::InvalidChar { kind, at, ch } => {
                write!(
                    f,
                    "{kind} has {ch:?} at position {at}, which is not allowed there"
                )
            }
        }
    }
}

impl std::error::Error for ParseIdentityError {}

/// Character rules for one identity kind.
struct Rules {
    kind: IdentityKind,
    max: usize,
    first: fn(char) -> bool,
    middle: fn(char) -> bool,
    last: fn(char) -> bool,
}

impl Rules {
    fn check(&self, s: &str) -> Result<(), ParseIdentityError> {
        let kind = self.kind;
        let len = s.chars().count();
        if len == 0 {
            return Err(ParseIdentityError::Empty(kind));
        }
        if len > self.max {
            return Err(ParseIdentityError::TooLong {
                kind,
                max: self.max,
                len,
            });
        }
        for (at, ch) in s.chars().enumerate() {
            let ok = (self.middle)(ch)
                && (at != 0 || (self.first)(ch))
                && (at + 1 != len || (self.last)(ch));
            if !ok {
                return Err(ParseIdentityError::InvalidChar { kind, at, ch });
            }
        }
        Ok(())
    }
}

fn upper_or_digit(ch: char) -> bool {
    ch.is_ascii_uppercase() || ch.is_ascii_digit()
}

fn lower_or_digit(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit()
}

/// Env-var prefix such as `OCTOBUDGET` or `SICKNESS_TRACKER`.
///
/// `A-Z`, `0-9` and `_`, starting with a letter and not ending with `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvPrefix(String);

impl EnvPrefix {
    const RULES: Rules = Rules {
        kind: IdentityKind::EnvPrefix,
        max: 32,
        first: |ch| ch.is_ascii_uppercase(),
        middle: |ch| upper_or_digit(ch) || ch == '_',
        last: upper_or_digit,
    };

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The full variable name for `suffix`, e.g. `OCTOBUDGET_GIT_SHA`.
    pub fn var(&self, suffix: &str) -> String {
        format!("{}_{suffix}", self.0)
    }
}

/// Log tag such as `octobudget` (logcat) or `sickness-tracker`.
///
/// ASCII letters, digits, `-`, `_` and `.`, at most 23 characters
/// (the classic Android logcat tag limit).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogTag(String);

impl LogTag {
    const RULES: Rules = Rules {
        kind: IdentityKind::LogTag,
        max: 23,
        first: |ch| ch.is_ascii_alphanumeric(),
        middle: |ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'),
        last: |ch| ch.is_ascii_alphanumeric(),
    };

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `localStorage` key prefix such as `sickness-tracker`.
///
/// `a-z`, `0-9` and `-`, starting with a letter and not ending with `-`.
/// Keys are `<prefix>.<name>`, e.g. `sickness-tracker.theme`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoragePrefix(String);

impl StoragePrefix {
    const RULES: Rules = Rules {
        kind: IdentityKind::StoragePrefix,
        max: 32,
        first: |ch| ch.is_ascii_lowercase(),
        middle: |ch| lower_or_digit(ch) || ch == '-',
        last: lower_or_digit,
    };

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The storage key for `name`, e.g. `sickness-tracker.theme`.
    pub fn key(&self, name: &str) -> String {
        format!("{}.{name}", self.0)
    }
}

macro_rules! identity_traits {
    ($($ty:ident),+) => {$(
        impl FromStr for $ty {
            type Err = ParseIdentityError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::RULES.check(s)?;
                Ok(Self(s.to_owned()))
            }
        }

        impl TryFrom<&str> for $ty {
            type Error = ParseIdentityError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    )+};
}

identity_traits!(EnvPrefix, LogTag, StoragePrefix);

#[cfg(test)]
mod tests;
