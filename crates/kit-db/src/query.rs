//! Run sea-query statements on a Turso connection, the union of both apps'
//! `db/query.rs`.
//!
//! Retrying on busy is app policy (OB's replica sync retries, ST doesn't), so
//! the pieces are split: [`build`] compiles once, and a [`Prepared`] can run
//! as many times as the app's retry loop wants. [`query`] and [`execute`] are
//! the one-shot forms. Failed statements are logged with their SQL and param
//! kinds (never values); busy failures only at `debug`, since the caller may
//! retry.

mod row;
mod values;

use std::future::Future;
use std::pin::Pin;

use sea_query::{QueryStatementWriter, SelectStatement, SqliteQueryBuilder};
use turso::transaction::{DropBehavior, Transaction};
use turso::{Connection, params_from_iter};

use crate::error::{Error, Result};

pub use row::{optional_f64, optional_i64, optional_text, required_f64, required_text};
pub use values::to_turso_values;

/// SQLite SQL plus its bound params, compiled from a sea-query statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    sql: String,
    params: Vec<turso::Value>,
}

/// Compile a sea-query AST to SQLite SQL + Turso params. Do this before
/// `.await`: sea-query statements are not `Send`.
pub fn build(statement: impl QueryStatementWriter) -> Result<Prepared> {
    let (sql, values) = statement.build(SqliteQueryBuilder);
    Ok(Prepared {
        sql,
        params: to_turso_values(values)?,
    })
}

impl Prepared {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn params(&self) -> &[turso::Value] {
        &self.params
    }

    /// Run as a query. Can be called again (e.g. by a busy-retry loop).
    pub async fn query(&self, conn: &Connection) -> Result<turso::Rows> {
        conn.query(&self.sql, params_from_iter(self.params.clone()))
            .await
            .map_err(|err| self.failed("db.query", err))
    }

    /// Run as a statement; returns the changed row count. Can be called again.
    pub async fn execute(&self, conn: &Connection) -> Result<u64> {
        conn.execute(&self.sql, params_from_iter(self.params.clone()))
            .await
            .map_err(|err| self.failed("db.execute", err))
    }

    fn failed(&self, what: &'static str, err: turso::Error) -> Error {
        let err = Error::from(err);
        let kinds = value_kinds(&self.params);
        if err.is_busy() {
            tracing::debug!(sql = %self.sql, params = %kinds, error = %err, "{what} busy");
        } else {
            tracing::error!(sql = %self.sql, params = %kinds, error = %err, "{what} failed");
        }
        err
    }
}

fn value_kinds(params: &[turso::Value]) -> String {
    params
        .iter()
        .map(|value| match value {
            turso::Value::Null => "null",
            turso::Value::Integer(_) => "int",
            turso::Value::Real(_) => "real",
            turso::Value::Text(_) => "text",
            turso::Value::Blob(_) => "blob",
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub async fn query(conn: &Connection, statement: SelectStatement) -> Result<turso::Rows> {
    build(statement)?.query(conn).await
}

pub async fn execute(conn: &Connection, statement: impl QueryStatementWriter) -> Result<u64> {
    build(statement)?.execute(conn).await
}

/// The closure [`with_transaction`] and [`in_transaction`] run. Generic over
/// the app's error, so call sites keep their own `?` conversions.
pub type TxFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// `BEGIN` on `conn`, via Turso's `unchecked_transaction` so call sites keep
/// `&Connection`. Split out so an app can retry just this step on busy.
pub async fn begin(conn: &Connection) -> Result<Transaction<'_>> {
    Ok(conn.unchecked_transaction().await?)
}

/// Run `f` inside `BEGIN`/`COMMIT`; roll back when `f` returns `Err`.
pub async fn with_transaction<T, E, F>(conn: &Connection, f: F) -> Result<T, E>
where
    E: From<Error>,
    F: for<'a> FnOnce(&'a Connection) -> TxFuture<'a, T, E>,
{
    in_transaction(begin(conn).await?, f).await
}

/// Run `f` in an already-begun `tx`, then commit, or roll back on `Err`.
///
/// The caller still sees `f`'s error when `f` returns `Err`. An engine-side
/// end (`ON CONFLICT ROLLBACK` and the like) leaves the connection usable.
pub async fn in_transaction<T, E, F>(tx: Transaction<'_>, f: F) -> Result<T, E>
where
    E: From<Error>,
    F: for<'a> FnOnce(&'a Connection) -> TxFuture<'a, T, E>,
{
    let mut tx = tx;
    // Rollback stays armed while `f` runs, so a cancel still rolls back later.
    let out = f(&tx).await;
    let end = match &out {
        Ok(_) => tx.execute("COMMIT", ()).await,
        Err(_) => tx.execute("ROLLBACK", ()).await,
    };
    // Autocommit means the engine, `f`, or the statement above already
    // finished the transaction. Disarm drop-rollback: a later `ROLLBACK`
    // would fail forever and never clear Turso's dangling flag. A still-open
    // transaction keeps `Rollback` armed.
    if tx.is_autocommit().unwrap_or(false) {
        tx.set_drop_behavior(DropBehavior::Ignore);
    }
    drop(tx);
    match (out, end) {
        (Ok(value), Ok(_)) => Ok(value),
        (Ok(_), Err(err)) => Err(Error::from(err).into()),
        (Err(err), Ok(_)) => Err(err),
        (Err(err), Err(rollback)) => {
            tracing::warn!(error = %rollback, "db.rollback failed");
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests;
