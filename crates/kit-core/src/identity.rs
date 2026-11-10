use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
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

/// The ASCII characters one position allows.
#[derive(Clone, Copy)]
struct Class {
    upper: bool,
    lower: bool,
    digit: bool,
    /// Extra ASCII punctuation.
    punct: &'static [u8],
}

impl Class {
    const fn allows(self, ch: char) -> bool {
        if !ch.is_ascii() {
            return false;
        }
        let byte = ch as u8;
        (self.upper && byte.is_ascii_uppercase())
            || (self.lower && byte.is_ascii_lowercase())
            || (self.digit && byte.is_ascii_digit())
            || contains(self.punct, byte)
    }
}

const fn contains(set: &[u8], byte: u8) -> bool {
    let mut i = 0;
    while i < set.len() {
        if set[i] == byte {
            return true;
        }
        i += 1;
    }
    false
}

const UPPER: Class = Class {
    upper: true,
    lower: false,
    digit: false,
    punct: b"",
};
const UPPER_OR_DIGIT: Class = Class {
    digit: true,
    ..UPPER
};
const LOWER: Class = Class {
    upper: false,
    lower: true,
    digit: false,
    punct: b"",
};
const LOWER_OR_DIGIT: Class = Class {
    digit: true,
    ..LOWER
};
const ALPHANUMERIC: Class = Class {
    upper: true,
    lower: true,
    digit: true,
    punct: b"",
};

/// Character rules for one identity kind. `const`, so a literal can be
/// checked at compile time (`literal`, [`crate::log_tag!`]).
struct Rules {
    kind: IdentityKind,
    max: usize,
    first: Class,
    middle: Class,
    last: Class,
}

impl Rules {
    const fn check(&self, s: &str) -> Result<(), ParseIdentityError> {
        let kind = self.kind;
        let bytes = s.as_bytes();
        let len = char_count(bytes);
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
        let mut i = 0;
        let mut at = 0;
        while i < bytes.len() {
            let (ch, width) = decode(bytes, i);
            let ok = self.middle.allows(ch)
                && (at != 0 || self.first.allows(ch))
                && (at + 1 != len || self.last.allows(ch));
            if !ok {
                return Err(ParseIdentityError::InvalidChar { kind, at, ch });
            }
            i += width;
            at += 1;
        }
        Ok(())
    }
}

/// `chars().count()` for `const`: every byte that is not a continuation.
const fn char_count(bytes: &[u8]) -> usize {
    let mut i = 0;
    let mut count = 0;
    while i < bytes.len() {
        if bytes[i] & 0xC0 != 0x80 {
            count += 1;
        }
        i += 1;
    }
    count
}

/// The `char` starting at byte `i` of valid UTF-8, and its width, for `const`
/// code (where `chars()` is not available). Only error reports need a
/// non-ASCII `char`.
const fn decode(bytes: &[u8], i: usize) -> (char, usize) {
    let lead = byte_at(bytes, i);
    let (code, width) = if lead < 0x80 {
        (lead, 1)
    } else if lead < 0xE0 {
        ((lead & 0x1F) << 6 | (byte_at(bytes, i + 1) & 0x3F), 2)
    } else if lead < 0xF0 {
        (
            (lead & 0x0F) << 12
                | (byte_at(bytes, i + 1) & 0x3F) << 6
                | (byte_at(bytes, i + 2) & 0x3F),
            3,
        )
    } else {
        (
            (lead & 0x07) << 18
                | (byte_at(bytes, i + 1) & 0x3F) << 12
                | (byte_at(bytes, i + 2) & 0x3F) << 6
                | (byte_at(bytes, i + 3) & 0x3F),
            4,
        )
    };
    match char::from_u32(code) {
        Some(ch) => (ch, width),
        None => (char::REPLACEMENT_CHARACTER, width),
    }
}

const fn byte_at(bytes: &[u8], i: usize) -> u32 {
    if i < bytes.len() { bytes[i] as u32 } else { 0 }
}

/// Env-var prefix such as `OCTOBUDGET` or `SICKNESS_TRACKER`.
///
/// `A-Z`, `0-9` and `_`, starting with a letter and not ending with `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvPrefix(Cow<'static, str>);

impl EnvPrefix {
    const RULES: Rules = Rules {
        kind: IdentityKind::EnvPrefix,
        max: 32,
        first: UPPER,
        middle: Class {
            punct: b"_",
            ..UPPER_OR_DIGIT
        },
        last: UPPER_OR_DIGIT,
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
pub struct LogTag(Cow<'static, str>);

impl LogTag {
    const RULES: Rules = Rules {
        kind: IdentityKind::LogTag,
        max: 23,
        first: ALPHANUMERIC,
        middle: Class {
            punct: b"-_.",
            ..ALPHANUMERIC
        },
        last: ALPHANUMERIC,
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
pub struct StoragePrefix(Cow<'static, str>);

impl StoragePrefix {
    const RULES: Rules = Rules {
        kind: IdentityKind::StoragePrefix,
        max: 32,
        first: LOWER,
        middle: Class {
            punct: b"-",
            ..LOWER_OR_DIGIT
        },
        last: LOWER_OR_DIGIT,
    };

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The storage key for `name`, e.g. `sickness-tracker.theme`.
    pub fn key(&self, name: &str) -> String {
        format!("{}.{name}", self.0)
    }
}

/// A `&'static str` already checked against `T`'s rules, from `T::literal`.
/// It is `Copy` with no drop, so `const` code can match on
/// `Result<Literal<T>, _>` (it cannot on a `Result<T, _>`, since `T` owns a
/// `Cow`). Use [`crate::log_tag!`] rather than this directly.
pub struct Literal<T>(&'static str, PhantomData<T>);

impl<T> Clone for Literal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Literal<T> {}

impl<T> fmt::Debug for Literal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0, f)
    }
}

impl<T> PartialEq for Literal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

macro_rules! identity_traits {
    ($($ty:ident),+) => {$(
        impl FromStr for $ty {
            type Err = ParseIdentityError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::RULES.check(s)?;
                Ok(Self(Cow::Owned(s.to_owned())))
            }
        }

        impl TryFrom<&str> for $ty {
            type Error = ParseIdentityError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                s.parse()
            }
        }

        impl $ty {
            #[doc = concat!("Checks a literal with the same rules as `parse`. `const`, so [`Literal`] can be built at compile time, then turned into a `", stringify!($ty), "` that cannot fail.")]
            pub const fn literal(s: &'static str) -> Result<Literal<Self>, ParseIdentityError> {
                match Self::RULES.check(s) {
                    Ok(()) => Ok(Literal(s, PhantomData)),
                    Err(err) => Err(err),
                }
            }

            /// The checked literal, borrowed rather than copied.
            pub const fn from_literal(literal: Literal<Self>) -> Self {
                Self(Cow::Borrowed(literal.0))
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
