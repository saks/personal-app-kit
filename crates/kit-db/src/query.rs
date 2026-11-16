//! Run sea-query statements on a Turso connection, the union of both apps'
//! `db/query.rs`.
//!
//! Callers hold a [`Conn`]. Clones share one Turso connection, including the
//! rollback flag a cancelled or panicking transaction leaves behind. A
//! transaction closure receives a [`Tx`] for that connection.
//!
//! Retrying on busy is app policy (OB's replica sync retries, ST doesn't), so
//! the pieces are split: [`build`] compiles once, and a [`Prepared`] can run
//! as many times as the app's retry loop wants. [`query`] and [`execute`] are
//! the one-shot forms. Both run the statement to completion: [`query`] returns
//! every row, and a busy or other execution error comes back from that call.
//! Failed statements are logged with their SQL and param kinds (never values);
//! busy failures only at `debug`, since the caller may retry.

mod row;
mod values;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sea_query::{QueryStatementWriter, SelectStatement, SqliteQueryBuilder};
use turso::IntoParams;
use turso::params_from_iter;
use turso::transaction::{DropBehavior, Transaction};

use crate::error::{Error, Result};

pub use row::{optional_f64, optional_i64, optional_text, required_f64, required_text};
pub use values::to_turso_values;

/// One Turso connection shared by every clone.
///
/// Cloning clones an `Arc`, so every clone is the same connection value. A
/// transaction dropped by cancel or panic records its rollback on that value.
/// The next statement on any clone runs the rollback first. Until that
/// statement, the transaction stays open and keeps its writer lock.
///
/// [`Conn::connection`] borrows the inner connection for raw SQL, pragma, and
/// sync. Clone this handle. Cloning the borrowed connection gives that clone
/// its own rollback flag: a write through it can disappear, or the original
/// handle can fail every later call.
#[derive(Clone, Debug)]
pub struct Conn {
    inner: Arc<turso::Connection>,
}

impl Conn {
    pub fn new(conn: turso::Connection) -> Self {
        Self {
            inner: Arc::new(conn),
        }
    }

    /// The inner connection for raw Turso calls.
    ///
    /// Clone [`Conn`] to share the connection. Cloning the returned value
    /// splits Turso's rollback flag from this handle.
    pub fn connection(&self) -> &turso::Connection {
        &self.inner
    }
}

/// A handle [`query`] and [`execute`] can run a statement on.
///
/// Implemented for `&Conn` and [`Tx`]. Apps do not implement this.
pub trait StatementConn: sealed::Sealed {
    #[doc(hidden)]
    fn statement_target(&self) -> StatementTarget<'_>;
}

/// Fields stay private so a caller cannot reach [`Conn::connection`] from a
/// [`Tx`]. The trait method has to be public; trait items share the trait's
/// visibility.
#[doc(hidden)]
pub struct StatementTarget<'a> {
    conn: &'a Conn,
    in_transaction: bool,
}

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for &Conn {}

impl StatementConn for &Conn {
    fn statement_target(&self) -> StatementTarget<'_> {
        StatementTarget {
            conn: self,
            in_transaction: false,
        }
    }
}

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

    /// Run the statement and return every row. A busy or other execution
    /// error comes back from this call. Can be called again (for example by
    /// a busy-retry loop).
    pub async fn query(&self, conn: impl StatementConn) -> Result<Vec<turso::Row>> {
        let target = conn.statement_target();
        if target.in_transaction {
            reject_transaction_control(&self.sql)?;
        }
        let rows = self.query_rows(target.conn).await?;
        collect_rows(rows)
            .await
            .map_err(|err| self.failed("db.query", err))
    }

    // Prepare and bind only. `query` steps, so execution errors reach `failed`.
    async fn query_rows(&self, conn: &Conn) -> Result<turso::Rows> {
        conn.connection()
            .query(&self.sql, params_from_iter(self.params.clone()))
            .await
            .map_err(|err| self.failed("db.query", err))
    }

    /// Run as a statement; returns the changed row count. Can be called again.
    pub async fn execute(&self, conn: impl StatementConn) -> Result<u64> {
        let target = conn.statement_target();
        if target.in_transaction {
            reject_transaction_control(&self.sql)?;
        }
        target
            .conn
            .connection()
            .execute(&self.sql, params_from_iter(self.params.clone()))
            .await
            .map_err(|err| self.failed("db.execute", err))
    }

    fn failed(&self, what: &'static str, err: turso::Error) -> Error {
        log_failed(what, &self.sql, &value_kinds(&self.params), err)
    }
}

fn value_kind(value: &turso::Value) -> &'static str {
    match value {
        turso::Value::Null => "null",
        turso::Value::Integer(_) => "int",
        turso::Value::Real(_) => "real",
        turso::Value::Text(_) => "text",
        turso::Value::Blob(_) => "blob",
    }
}

fn value_kinds(params: &[turso::Value]) -> String {
    params.iter().map(value_kind).collect::<Vec<_>>().join(",")
}

fn params_kinds(params: &turso::params::Params) -> String {
    match params {
        turso::params::Params::None => String::new(),
        turso::params::Params::Positional(values) => value_kinds(values),
        turso::params::Params::Named(values) => values
            .iter()
            .map(|(_, value)| value_kind(value))
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn log_failed(what: &'static str, sql: &str, kinds: &str, err: turso::Error) -> Error {
    let err = Error::from(err);
    if err.is_busy() {
        tracing::debug!(sql = %sql, params = %kinds, error = %err, "{what} busy");
    } else {
        tracing::error!(sql = %sql, params = %kinds, error = %err, "{what} failed");
    }
    err
}

async fn collect_rows(mut rows: turso::Rows) -> std::result::Result<Vec<turso::Row>, turso::Error> {
    let mut out = Vec::new();
    loop {
        match rows.next().await {
            Ok(Some(row)) => out.push(row),
            Ok(None) => return Ok(out),
            Err(err) => return Err(err),
        }
    }
}

pub async fn query(
    conn: impl StatementConn,
    statement: SelectStatement,
) -> Result<Vec<turso::Row>> {
    build(statement)?.query(conn).await
}

pub async fn execute(
    conn: impl StatementConn,
    statement: impl QueryStatementWriter,
) -> Result<u64> {
    build(statement)?.execute(conn).await
}

/// A connection inside a kit transaction.
///
/// [`with_transaction`] and [`in_transaction`] pass this to the closure. It
/// runs statements on the open transaction and has no [`Conn::connection`].
///
/// A [`Conn`] captured from outside the closure is still the app's handle.
/// [`begin`] on it returns [`Error::NestedTransaction`].
///
/// ```compile_fail
/// fn no_connection(tx: kit_db::query::Tx<'_>) {
///     let _ = tx.connection();
/// }
/// ```
///
/// ```compile_fail
/// fn no_begin(tx: kit_db::query::Tx<'_>) {
///     let _ = kit_db::query::begin(tx);
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Tx<'a> {
    conn: &'a Conn,
}

impl sealed::Sealed for Tx<'_> {}

impl StatementConn for Tx<'_> {
    fn statement_target(&self) -> StatementTarget<'_> {
        StatementTarget {
            conn: self.conn,
            in_transaction: true,
        }
    }
}

impl Tx<'_> {
    /// Run raw SQL inside this transaction. Returns the changed row count.
    ///
    /// `BEGIN`, `COMMIT`, `END`, `ROLLBACK`, `SAVEPOINT`, and `RELEASE`
    /// return [`Error::TransactionControl`] and the transaction stays open.
    pub async fn execute_raw(&self, sql: &str, params: impl IntoParams) -> Result<u64> {
        reject_transaction_control(sql)?;
        raw_execute(self.conn, sql, params).await
    }

    /// Run a raw query inside this transaction and return every row.
    ///
    /// Transaction-control statements return [`Error::TransactionControl`].
    pub async fn query_raw(&self, sql: &str, params: impl IntoParams) -> Result<Vec<turso::Row>> {
        reject_transaction_control(sql)?;
        raw_query(self.conn, sql, params).await
    }
}

async fn raw_execute(conn: &Conn, sql: &str, params: impl IntoParams) -> Result<u64> {
    let params = params.into_params().map_err(Error::from)?;
    let kinds = params_kinds(&params);
    conn.connection()
        .execute(sql, params)
        .await
        .map_err(|err| log_failed("db.execute", sql, &kinds, err))
}

async fn raw_query(conn: &Conn, sql: &str, params: impl IntoParams) -> Result<Vec<turso::Row>> {
    let params = params.into_params().map_err(Error::from)?;
    let kinds = params_kinds(&params);
    let rows = conn
        .connection()
        .query(sql, params)
        .await
        .map_err(|err| log_failed("db.query", sql, &kinds, err))?;
    collect_rows(rows)
        .await
        .map_err(|err| log_failed("db.query", sql, &kinds, err))
}

const TRANSACTION_CONTROL: &[&str] =
    &["BEGIN", "COMMIT", "END", "ROLLBACK", "SAVEPOINT", "RELEASE"];

fn reject_transaction_control(sql: &str) -> Result<()> {
    let Some(keyword) = first_keyword(sql) else {
        return Ok(());
    };
    if TRANSACTION_CONTROL
        .iter()
        .any(|word| keyword.eq_ignore_ascii_case(word))
    {
        Err(Error::TransactionControl)
    } else {
        Ok(())
    }
}

/// The first keyword, after leading whitespace and one `--` or `/* */` comment.
fn first_keyword(sql: &str) -> Option<&str> {
    let sql = sql.trim_start();
    let sql = strip_one_leading_comment(sql).trim_start();
    let end = sql
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(sql.len());
    let word = &sql[..end];
    if word.is_empty() { None } else { Some(word) }
}

fn strip_one_leading_comment(sql: &str) -> &str {
    if let Some(rest) = sql.strip_prefix("--") {
        return rest.split_once('\n').map(|(_, rest)| rest).unwrap_or("");
    }
    if let Some(rest) = sql.strip_prefix("/*") {
        return rest.split_once("*/").map(|(_, rest)| rest).unwrap_or("");
    }
    sql
}

/// The closure [`with_transaction`] and [`in_transaction`] run. Generic over
/// the app's error, so call sites keep their own `?` conversions.
pub type TxFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// `BEGIN` on `conn`. Split out so an app can retry just this step on busy.
///
/// The returned transaction borrows `conn`. Dropping it leaves the rollback
/// pending until the next statement on any clone of `conn`.
///
/// `BEGIN` while a transaction is already open returns
/// [`Error::NestedTransaction`]. turso 0.8.3 documents that nested error as
/// unspecified; this matches the string this pin returns. A dangling rollback
/// from a cancelled transaction is applied before `BEGIN`, so the next
/// `begin` after a cancel starts a new transaction.
pub async fn begin(conn: &Conn) -> Result<Transaction<'_>> {
    match conn.connection().unchecked_transaction().await {
        Ok(tx) => Ok(tx),
        Err(err) if is_nested_transaction(&err) => Err(Error::NestedTransaction),
        Err(err) => Err(err.into()),
    }
}

/// turso 0.8.3 documents the nested-transaction error as unspecified. This
/// pin returns this text from `BEGIN` while a transaction is open.
fn is_nested_transaction(err: &turso::Error) -> bool {
    err.to_string()
        .to_lowercase()
        .contains("cannot start a transaction within a transaction")
}

/// Run `f` inside `BEGIN`/`COMMIT`. Roll back when `f` returns `Err`.
///
/// The closure receives a [`Tx`] for `conn`. Pass that value to [`query`],
/// [`execute`], [`Prepared::query`], and [`Prepared::execute`]. Raw SQL is
/// [`Tx::execute_raw`] and [`Tx::query_raw`].
///
/// If this future is dropped, or `f` panics, the transaction stays open and
/// keeps its writer lock until the next statement on any clone of `conn`.
/// That statement rolls the transaction back and then runs. Clone [`Conn`].
/// Cloning [`Conn::connection`] leaves that clone with its own rollback flag.
pub async fn with_transaction<T, E, F>(conn: &Conn, f: F) -> Result<T, E>
where
    E: From<Error>,
    F: for<'a> FnOnce(Tx<'a>) -> TxFuture<'a, T, E>,
{
    let conn = conn.clone();
    let tx = begin(&conn).await?;
    in_transaction(&conn, tx, f).await
}

/// Run `f` in an already-begun `tx`, then commit, or roll back on `Err`.
///
/// `conn` is the [`Conn`] [`begin`] borrowed. The closure receives a [`Tx`]
/// for that handle.
///
/// The caller still sees `f`'s error when `f` returns `Err`. An engine-side
/// end (`ON CONFLICT ROLLBACK` and the like) leaves the connection usable.
///
/// If this future is dropped, or `f` panics, the transaction stays open until
/// the next statement on any clone of `conn`. That statement rolls it back
/// and then runs.
pub async fn in_transaction<T, E, F>(conn: &Conn, tx: Transaction<'_>, f: F) -> Result<T, E>
where
    E: From<Error>,
    F: for<'a> FnOnce(Tx<'a>) -> TxFuture<'a, T, E>,
{
    let mut tx = tx;
    // Rollback stays armed while `f` runs. A cancel records it on this `Conn`
    // value; the next statement on any clone applies it.
    let out = f(Tx { conn }).await;
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
