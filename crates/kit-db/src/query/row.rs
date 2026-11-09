//! Typed column readers, lenient the way both apps' stored data needs:
//! numbers stored as text parse, `NULL` text reads as empty.

use crate::error::{Error, Result};

/// Text, with blank (empty or whitespace) as `None`.
pub fn optional_text(row: &turso::Row, idx: usize) -> Result<Option<String>> {
    let value = required_text(row, idx)?;
    if value.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Text; `NULL` reads as empty and numbers as their decimal form.
pub fn required_text(row: &turso::Row, idx: usize) -> Result<String> {
    match row.get_value(idx)? {
        turso::Value::Null => Ok(String::new()),
        turso::Value::Text(value) => Ok(value),
        turso::Value::Integer(value) => Ok(value.to_string()),
        turso::Value::Real(value) => Ok(value.to_string()),
        turso::Value::Blob(bytes) => Ok(String::from_utf8(bytes)?),
    }
}

/// A number; `NULL` reads as `0.0`.
pub fn required_f64(row: &turso::Row, idx: usize) -> Result<f64> {
    Ok(optional_f64(row, idx)?.unwrap_or(0.0))
}

pub fn optional_f64(row: &turso::Row, idx: usize) -> Result<Option<f64>> {
    match row.get_value(idx)? {
        turso::Value::Null => Ok(None),
        turso::Value::Real(value) => Ok(Some(value)),
        // Same as SQLite's own INTEGER → REAL affinity.
        turso::Value::Integer(value) => Ok(Some(value as f64)),
        turso::Value::Text(value) => value.parse().map(Some).map_err(|_| Error::NotANumber),
        turso::Value::Blob(_) => Err(Error::NotANumber),
    }
}

pub fn optional_i64(row: &turso::Row, idx: usize) -> Result<Option<i64>> {
    match row.get_value(idx)? {
        turso::Value::Null => Ok(None),
        turso::Value::Integer(value) => Ok(Some(value)),
        // Truncates toward zero and saturates, as ST's `as i64` did.
        turso::Value::Real(value) => Ok(Some(value as i64)),
        turso::Value::Text(value) => value.parse().map(Some).map_err(|_| Error::NotAnInteger),
        turso::Value::Blob(_) => Err(Error::NotAnInteger),
    }
}
