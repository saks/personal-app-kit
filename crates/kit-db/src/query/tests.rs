use sea_query::{Alias, Enum, Expr, ExprTrait, OptionEnum, Query, Value, Values};

use super::*;

/// What an app's own error looks like to [`with_transaction`].
#[derive(Debug, PartialEq)]
struct AppError(String);

impl From<Error> for AppError {
    fn from(err: Error) -> Self {
        AppError(err.to_string())
    }
}

async fn memory_conn() -> Connection {
    let db = turso::Builder::new_local(":memory:").build().await.unwrap();
    db.connect().unwrap()
}

async fn count(conn: &Connection) -> i64 {
    let mut rows = conn.query("SELECT COUNT(*) FROM t", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    optional_i64(&row, 0).unwrap().unwrap()
}

async fn one_row(conn: &Connection, sql: &str) -> turso::Row {
    let mut rows = conn.query(sql, ()).await.unwrap();
    rows.next().await.unwrap().unwrap()
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
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", ())
        .await
        .unwrap();
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
    let mut rows = query(&conn, select).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(optional_i64(&row, 0).unwrap(), Some(5));
}

#[tokio::test]
async fn a_prepared_statement_runs_more_than_once() {
    let conn = memory_conn().await;
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", ())
        .await
        .unwrap();
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

#[tokio::test]
async fn with_transaction_rolls_back_on_error() {
    let conn = memory_conn().await;
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", ())
        .await
        .unwrap();
    let err = with_transaction(&conn, |tx| {
        Box::pin(async move {
            tx.execute("INSERT INTO t (n) VALUES (1)", ())
                .await
                .map_err(Error::from)?;
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
        let conn = db.connect().unwrap();
        conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", ())
            .await
            .unwrap();
        let kept: Result<i64, AppError> = with_transaction(&conn, |tx| {
            Box::pin(async move {
                tx.execute("INSERT INTO t (n) VALUES (1)", ())
                    .await
                    .map_err(Error::from)?;
                Ok(1)
            })
        })
        .await;
        assert_eq!(kept, Ok(1));
        let dropped: Result<(), AppError> = with_transaction(&conn, |tx| {
            Box::pin(async move {
                tx.execute("INSERT INTO t (n) VALUES (2)", ())
                    .await
                    .map_err(Error::from)?;
                tx.execute("INSERT INTO t (n) VALUES (3)", ())
                    .await
                    .map_err(Error::from)?;
                Err(AppError("boom".into()))
            })
        })
        .await;
        assert_eq!(dropped, Err(AppError("boom".into())));
    }
    let db = turso::Builder::new_local(path).build().await.unwrap();
    let conn = db.connect().unwrap();
    assert_eq!(count(&conn).await, 1);
    let row = one_row(&conn, "SELECT n FROM t").await;
    assert_eq!(optional_i64(&row, 0).unwrap(), Some(1));
}

#[tokio::test]
async fn begin_then_in_transaction_commits() {
    let conn = memory_conn().await;
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, n INTEGER)", ())
        .await
        .unwrap();
    let tx = begin(&conn).await.unwrap();
    let out: Result<(), Error> = in_transaction(tx, |tx| {
        Box::pin(async move {
            tx.execute("INSERT INTO t (n) VALUES (9)", ()).await?;
            Ok(())
        })
    })
    .await;
    assert!(out.is_ok());
    assert_eq!(count(&conn).await, 1);
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
