//! sea-query bound values → Turso values.

use sea_query::{Value, Values};

use crate::error::{Error, Result};

pub fn to_turso_values(values: Values) -> Result<Vec<turso::Value>> {
    values.into_iter().map(to_turso_value).collect()
}

fn to_turso_value(value: Value) -> Result<turso::Value> {
    match value {
        Value::Bool(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::TinyInt(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::SmallInt(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::Int(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::BigInt(Some(v)) => Ok(turso::Value::Integer(v)),
        Value::TinyUnsigned(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::SmallUnsigned(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::Unsigned(Some(v)) => Ok(turso::Value::Integer(i64::from(v))),
        Value::BigUnsigned(Some(v)) => i64::try_from(v)
            .map(turso::Value::Integer)
            .map_err(|_| Error::IntegerOverflow),
        Value::Float(Some(v)) => Ok(turso::Value::Real(f64::from(v))),
        Value::Double(Some(v)) => Ok(turso::Value::Real(v)),
        Value::String(Some(v)) => Ok(turso::Value::Text(v)),
        Value::Char(Some(v)) => Ok(turso::Value::Text(v.to_string())),
        Value::Bytes(Some(v)) => Ok(turso::Value::Blob(v)),
        Value::Bool(None)
        | Value::TinyInt(None)
        | Value::SmallInt(None)
        | Value::Int(None)
        | Value::BigInt(None)
        | Value::TinyUnsigned(None)
        | Value::SmallUnsigned(None)
        | Value::Unsigned(None)
        | Value::BigUnsigned(None)
        | Value::Float(None)
        | Value::Double(None)
        | Value::String(None)
        | Value::Char(None)
        | Value::Bytes(None) => Ok(turso::Value::Null),
        Value::Enum(sea_query::OptionEnum::None(_)) => Ok(turso::Value::Null),
        Value::Enum(sea_query::OptionEnum::Some(_)) => Err(Error::UnsupportedEnum),
    }
}
