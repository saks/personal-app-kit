use std::future::Future;

use sea_query::{Alias, Enum, Expr, ExprTrait, OptionEnum, Order, Query, Value, Values};

use super::*;

/// What an app's own error looks like to [`with_transaction`].
#[derive(Debug, PartialEq)]
struct AppError(String);

impl From<Error> for AppError {
    fn from(err: Error) -> Self {
        AppError(err.to_string())
    }
}

async fn memory_conn() -> Conn {
    let db = turso::Builder::new_local(":memory:").build().await.unwrap();
    Conn::new(db.connect().unwrap())
}

async fn exec(conn: &Conn, sql: &str) {
    conn.connection().execute(sql, ()).await.unwrap();
}

async fn count(conn: &Conn) -> i64 {
    let mut rows = conn
        .connection()
        .query("SELECT COUNT(*) FROM t", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    optional_i64(&row, 0).unwrap().unwrap()
}

async fn nums(conn: &Conn) -> Vec<i64> {
    let mut rows = conn
        .connection()
        .query("SELECT n FROM t ORDER BY n", ())
        .await
        .unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push(optional_i64(&row, 0).unwrap().unwrap());
    }
    out
}

async fn one_row(conn: &Conn, sql: &str) -> turso::Row {
    let mut rows = conn.connection().query(sql, ()).await.unwrap();
    rows.next().await.unwrap().unwrap()
}

/// Drop `with_transaction` after it has inserted `1` and parked.
async fn cancel_after_insert(conn: &Conn) {
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    let work = with_transaction(conn, move |tx| {
        let flag = flag.clone();
        Box::pin(async move {
            tx.execute_raw("INSERT INTO t (n) VALUES (1)", ()).await?;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            std::future::pending::<Result<(), AppError>>().await
        })
    });
    let capped = tokio::time::timeout(std::time::Duration::from_secs(2), async move {
        // Leaving this block drops `work`, which is the cancel.
        {
            let mut work = std::pin::pin!(work);
            for _ in 0..1000 {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let step =
                    std::future::poll_fn(|cx| std::task::Poll::Ready(work.as_mut().poll(cx))).await;
                assert!(
                    matches!(step, std::task::Poll::Pending),
                    "transaction finished"
                );
                tokio::task::yield_now().await;
            }
            assert!(
                done.load(std::sync::atomic::Ordering::SeqCst),
                "insert never finished"
            );
        }
    });
    capped.await.expect("cancel timed out");
}

#[test]
fn converts_bound_types() {
    let values = Values(vec![
        Value::String(Some("cad".into())),
        Value::BigInt(Some(7)),
        Value::Double(Some(1.5)),
        Value::String(None),
        Value::Bool(Some(true)),
        Value::Int(Some(3)),
    ]);
    let out = to_turso_values(values).unwrap();
    assert_eq!(out[0], turso::Value::Text("cad".into()));
    assert_eq!(out[1], turso::Value::Integer(7));
    assert_eq!(out[2], turso::Value::Real(1.5));
    assert_eq!(out[3], turso::Value::Null);
    assert_eq!(out[4], turso::Value::Integer(1));
    assert_eq!(out[5], turso::Value::Integer(3));
}

#[test]
fn rejects_enum_values_and_u64_overflow() {
    let values = Values(vec![Value::Enum(OptionEnum::Some(Box::new(Enum {
        type_name: "x".into(),
        value: "y".into(),
    })))]);
    assert!(matches!(
        to_turso_values(values),
        Err(Error::UnsupportedEnum)
    ));
    let big = Values(vec![Value::BigUnsigned(Some(u64::MAX))]);
    assert!(matches!(to_turso_values(big), Err(Error::IntegerOverflow)));
}

#[test]
fn rejects_values_from_sea_query_type_features() {
    // `with-json` is on for tests only (see kit-db's dev-dependencies).
    let json = Values(vec![Value::Json(Some(Box::new(serde_json::json!(1))))]);
    assert!(matches!(
        to_turso_values(json),
        Err(Error::UnsupportedValue)
    ));
}

#[test]
fn build_compiles_sql_and_params() {
    let prepared = build(
        Query::select()
            .expr(Expr::val(7))
            .and_where(Expr::val("a").eq(Expr::val("a")))
            .to_owned(),
    )
    .unwrap();
    assert_eq!(prepared.sql(), r#"SELECT ? WHERE ? = ?"#);
    assert_eq!(prepared.params().len(), 3);
}

#[tokio::test]
async fn query_and_execute_run_built_statements() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    let insert = Query::insert()
        .into_table(Alias::new("t"))
        .columns([Alias::new("n")])
        .values_panic([5.into()])
        .to_owned();
    assert_eq!(execute(&conn, insert).await.unwrap(), 1);
    let select = Query::select()
        .column(Alias::new("n"))
        .from(Alias::new("t"))
        .to_owned();
    let rows = query(&conn, select).await.unwrap();
    let row = rows.first().unwrap();
    assert_eq!(optional_i64(row, 0).unwrap(), Some(5));
}

#[tokio::test]
async fn query_returns_every_row() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    exec(&conn, "INSERT INTO t (n) VALUES (1), (2)").await;
    let select = Query::select()
        .column(Alias::new("n"))
        .from(Alias::new("t"))
        .order_by(Alias::new("n"), Order::Asc)
        .to_owned();
    let rows = query(&conn, select).await.unwrap();
    let nums: Vec<_> = rows
        .iter()
        .map(|row| optional_i64(row, 0).unwrap().unwrap())
        .collect();
    assert_eq!(nums, vec![1, 2]);
}

#[tokio::test]
async fn a_prepared_statement_runs_more_than_once() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    let insert = build(
        Query::insert()
            .into_table(Alias::new("t"))
            .columns([Alias::new("n")])
            .values_panic([1.into()])
            .to_owned(),
    )
    .unwrap();
    insert.execute(&conn).await.unwrap();
    insert.execute(&conn).await.unwrap();
    assert_eq!(count(&conn).await, 2);
}

#[tokio::test]
async fn sql_errors_come_back_typed_and_not_busy() {
    let conn = memory_conn().await;
    let select = Query::select()
        .column(Alias::new("n"))
        .from(Alias::new("missing"))
        .to_owned();
    let Err(err) = query(&conn, select).await else {
        panic!("a missing table must fail");
    };
    assert!(matches!(err, Error::Turso(_)), "{err:?}");
    assert!(!err.is_busy());
}

/// `abs` of `i64::MIN` prepares successfully and fails when the statement is
/// stepped. That error has to come back from `query`.
#[tokio::test]
async fn execution_errors_come_back_from_query() {
    let conn = memory_conn().await;
    let select = Query::select()
        .expr(Expr::cust("abs(-9223372036854775808)"))
        .to_owned();
    let Err(err) = query(&conn, select).await else {
        panic!("integer overflow must fail when the statement is stepped");
    };
    assert!(matches!(err, Error::Turso(_)), "{err:?}");
    assert!(!err.is_busy());
    assert!(
        err.to_string().to_lowercase().contains("integer overflow"),
        "{err}"
    );
}

#[tokio::test]
async fn with_transaction_rolls_back_on_error() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    let err = with_transaction(&conn, |tx| {
        Box::pin(async move {
            tx.execute_raw("INSERT INTO t (n) VALUES (1)", ()).await?;
            Err::<(), _>(AppError("boom".into()))
        })
    })
    .await
    .unwrap_err();
    assert_eq!(err, AppError("boom".into()));
    assert_eq!(count(&conn).await, 0);
}

/// Acceptance (#97): rollback against a temp file, seen from a fresh
/// connection after reopening the database.
#[tokio::test]
async fn with_transaction_on_a_temp_file_commits_ok_and_rolls_back_err() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kit-db-test.db");
    let path = path.to_str().unwrap();
    {
        let db = turso::Builder::new_local(path).build().await.unwrap();
        let conn = Conn::new(db.connect().unwrap());
        exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
        let kept: Result<i64, AppError> = with_transaction(&conn, |tx| {
            Box::pin(async move {
                tx.execute_raw("INSERT INTO t (n) VALUES (1)", ()).await?;
                Ok(1)
            })
        })
        .await;
        assert_eq!(kept, Ok(1));
        let dropped: Result<(), AppError> = with_transaction(&conn, |tx| {
            Box::pin(async move {
                tx.execute_raw("INSERT INTO t (n) VALUES (2)", ()).await?;
                tx.execute_raw("INSERT INTO t (n) VALUES (3)", ()).await?;
                Err(AppError("boom".into()))
            })
        })
        .await;
        assert_eq!(dropped, Err(AppError("boom".into())));
    }
    let db = turso::Builder::new_local(path).build().await.unwrap();
    let conn = Conn::new(db.connect().unwrap());
    assert_eq!(count(&conn).await, 1);
    let row = one_row(&conn, "SELECT n FROM t").await;
    assert_eq!(optional_i64(&row, 0).unwrap(), Some(1));
}

/// `ON CONFLICT ROLLBACK` ends the transaction inside the engine. The
/// connection must still answer the next query, and the caller must still
/// see the constraint error.
#[tokio::test]
async fn engine_side_rollback_leaves_the_connection_usable() {
    let conn = memory_conn().await;
    exec(
        &conn,
        "CREATE TABLE t (n INTEGER UNIQUE ON CONFLICT ROLLBACK)",
    )
    .await;
    exec(&conn, "INSERT INTO t (n) VALUES (1)").await;
    let err = with_transaction(&conn, |tx| {
        Box::pin(async move {
            tx.execute_raw("INSERT INTO t (n) VALUES (1)", ()).await?;
            Ok::<(), AppError>(())
        })
    })
    .await
    .unwrap_err();
    assert_eq!(err, AppError("UNIQUE constraint failed: t.n".into()));
    assert_eq!(count(&conn).await, 1);
}

/// A deferred foreign key makes `COMMIT` fail while the transaction is still
/// open. The caller sees that error, the row is not kept, and the connection
/// still answers the next query.
#[tokio::test]
async fn failed_commit_leaves_the_connection_usable() {
    let conn = memory_conn().await;
    exec(&conn, "PRAGMA foreign_keys = ON").await;
    exec(&conn, "CREATE TABLE parent (id INTEGER PRIMARY KEY)").await;
    exec(
        &conn,
        "CREATE TABLE child (
            id INTEGER PRIMARY KEY,
            pid INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED
        )",
    )
    .await;
    let inserted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = inserted.clone();
    let err = with_transaction(&conn, move |tx| {
        Box::pin(async move {
            tx.execute_raw("INSERT INTO child (id, pid) VALUES (1, 99)", ())
                .await?;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok::<(), AppError>(())
        })
    })
    .await
    .unwrap_err();
    assert_eq!(err, AppError("FOREIGN KEY constraint failed".into()));
    assert!(
        inserted.load(std::sync::atomic::Ordering::SeqCst),
        "insert never finished"
    );
    let child = one_row(&conn, "SELECT COUNT(*) FROM child").await;
    assert_eq!(optional_i64(&child, 0).unwrap(), Some(0));
    exec(&conn, "INSERT INTO parent (id) VALUES (1)").await;
    let parent = one_row(&conn, "SELECT COUNT(*) FROM parent").await;
    assert_eq!(optional_i64(&parent, 0).unwrap(), Some(1));
}

#[tokio::test]
async fn begin_then_in_transaction_commits() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    let tx = begin(&conn).await.unwrap();
    let out: Result<(), Error> = in_transaction(&conn, tx, |tx| {
        Box::pin(async move {
            tx.execute_raw("INSERT INTO t (n) VALUES (9)", ()).await?;
            Ok(())
        })
    })
    .await;
    assert!(out.is_ok());
    assert_eq!(count(&conn).await, 1);
}

#[test]
fn transaction_control_keywords_are_rejected() {
    for sql in [
        "COMMIT",
        "  begin deferred",
        "-- x\nROLLBACK",
        "/* c */ release s",
        "SAVEPOINT s",
        "END",
    ] {
        assert!(
            matches!(
                reject_transaction_control(sql),
                Err(Error::TransactionControl)
            ),
            "{sql}"
        );
    }
    assert!(reject_transaction_control("INSERT INTO t VALUES (1)").is_ok());
    assert!(reject_transaction_control("select 1").is_ok());
}

#[tokio::test]
async fn a_second_begin_is_a_nested_transaction() {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    let tx = begin(&conn).await.unwrap();
    let err = begin(&conn).await.unwrap_err();
    assert!(matches!(err, Error::NestedTransaction), "{err:?}");
    drop(tx);
    exec(&conn, "INSERT INTO t (n) VALUES (1)").await;
    assert_eq!(count(&conn).await, 1);
}

#[tokio::test]
async fn with_transaction_after_cancel_commits() {
    let conn = table().await;
    cancel_after_insert(&conn).await;
    let kept: Result<(), AppError> = with_transaction(&conn, |tx| {
        Box::pin(async move {
            tx.execute_raw("INSERT INTO t (n) VALUES (2)", ()).await?;
            Ok(())
        })
    })
    .await;
    assert_eq!(kept, Ok(()));
    assert_eq!(nums(&conn).await, vec![2]);
}

#[tokio::test]
async fn transaction_control_sql_is_rejected_and_a_later_insert_commits() {
    let conn = table().await;
    let out: Result<(), Error> = with_transaction(&conn, |tx| {
        Box::pin(async move {
            let commit = tx.execute_raw("COMMIT", ()).await.unwrap_err();
            assert!(matches!(commit, Error::TransactionControl), "{commit:?}");
            let begin = tx.execute_raw("  begin deferred", ()).await.unwrap_err();
            assert!(matches!(begin, Error::TransactionControl), "{begin:?}");
            tx.execute_raw("INSERT INTO t (n) VALUES (4)", ()).await?;
            Ok(())
        })
    })
    .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(nums(&conn).await, vec![4]);
}

#[tokio::test]
async fn statements_on_tx_commit() {
    let conn = table().await;
    let insert = Query::insert()
        .into_table(Alias::new("t"))
        .columns([Alias::new("n")])
        .values_panic([6.into()])
        .to_owned();
    let prepared = build(insert.clone()).unwrap();
    let out: Result<(), Error> = with_transaction(&conn, |tx| {
        Box::pin(async move {
            execute(tx, insert).await?;
            prepared.execute(tx).await?;
            let rows = tx.query_raw("SELECT n FROM t", ()).await?;
            let nums: Vec<_> = rows
                .iter()
                .map(|row| optional_i64(row, 0).unwrap().unwrap())
                .collect();
            assert_eq!(nums, vec![6, 6]);
            Ok(())
        })
    })
    .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(nums(&conn).await, vec![6, 6]);
}

#[tokio::test]
async fn commit_on_a_conn_is_not_transaction_control() {
    let conn = memory_conn().await;
    let err = Error::from(conn.connection().execute("COMMIT", ()).await.unwrap_err());
    assert!(matches!(err, Error::Turso(_)), "{err:?}");
}

async fn table() -> Conn {
    let conn = memory_conn().await;
    exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
    conn
}

async fn later_write_survives(conn: &Conn, cloned: &Conn) {
    exec(cloned, "INSERT INTO t (n) VALUES (2)").await;
    assert_eq!(nums(cloned).await, vec![2]);
    assert_eq!(nums(conn).await, vec![2]);
}

#[tokio::test]
async fn cancelled_transaction_keeps_a_later_write_on_a_clone_taken_before() {
    let conn = table().await;
    let cloned = conn.clone();
    cancel_after_insert(&conn).await;
    later_write_survives(&conn, &cloned).await;
}

#[tokio::test]
async fn cancelled_transaction_keeps_a_later_write_on_a_clone_taken_after() {
    let conn = table().await;
    cancel_after_insert(&conn).await;
    let cloned = conn.clone();
    later_write_survives(&conn, &cloned).await;
}

#[tokio::test]
async fn panicking_transaction_keeps_a_later_write_on_a_surviving_clone() {
    let conn = table().await;
    let surviving = conn.clone();
    let join = tokio::spawn(async move {
        let _ = with_transaction(&conn, |tx| {
            Box::pin(async move {
                tx.execute_raw("INSERT INTO t (n) VALUES (1)", ()).await?;
                panic!("closure panic");
                #[allow(unreachable_code)]
                Ok::<(), AppError>(())
            })
        })
        .await;
    });
    assert!(join.await.is_err());
    later_write_survives(&surviving, &surviving).await;
}

#[tokio::test]
async fn cancelled_task_rolls_back_and_a_survivor_write_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kit-db-cancel.db");
    let path = path.to_str().unwrap().to_owned();
    {
        let db = turso::Builder::new_local(&path).build().await.unwrap();
        let conn = Conn::new(db.connect().unwrap());
        exec(&conn, "CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)").await;
        let survivor = conn.clone();
        let task_conn = conn.clone();
        let join = tokio::spawn(async move {
            cancel_after_insert(&task_conn).await;
        });
        join.await.unwrap();
        exec(&survivor, "INSERT INTO t (n) VALUES (2)").await;
        assert_eq!(nums(&survivor).await, vec![2]);
    }
    let db = turso::Builder::new_local(&path).build().await.unwrap();
    let conn = Conn::new(db.connect().unwrap());
    assert_eq!(nums(&conn).await, vec![2]);
}

#[tokio::test]
async fn row_readers_are_lenient_like_the_apps() {
    let conn = memory_conn().await;
    let row = one_row(
        &conn,
        "SELECT NULL, '  ', 'hi', 42, 1.5, '2.5', '7', 'x', x'6869', x'ff'",
    )
    .await;
    assert_eq!(required_text(&row, 0).unwrap(), "");
    assert_eq!(optional_text(&row, 1).unwrap(), None);
    assert_eq!(optional_text(&row, 2).unwrap().as_deref(), Some("hi"));
    assert_eq!(required_text(&row, 3).unwrap(), "42");
    assert_eq!(required_text(&row, 8).unwrap(), "hi");
    assert!(matches!(required_text(&row, 9), Err(Error::NotUtf8(_))));

    assert_eq!(required_f64(&row, 0).unwrap(), 0.0);
    assert_eq!(optional_f64(&row, 0).unwrap(), None);
    assert_eq!(optional_f64(&row, 3).unwrap(), Some(42.0));
    assert_eq!(required_f64(&row, 4).unwrap(), 1.5);
    assert_eq!(optional_f64(&row, 5).unwrap(), Some(2.5));
    assert!(matches!(optional_f64(&row, 7), Err(Error::NotANumber)));
    assert!(matches!(optional_f64(&row, 8), Err(Error::NotANumber)));

    assert_eq!(optional_i64(&row, 0).unwrap(), None);
    assert_eq!(optional_i64(&row, 3).unwrap(), Some(42));
    assert_eq!(optional_i64(&row, 4).unwrap(), Some(1));
    assert_eq!(optional_i64(&row, 6).unwrap(), Some(7));
    assert!(matches!(optional_i64(&row, 7), Err(Error::NotAnInteger)));
    assert!(matches!(optional_i64(&row, 8), Err(Error::NotAnInteger)));
}
