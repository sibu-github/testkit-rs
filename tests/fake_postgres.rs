#![cfg(feature = "postgres")]

use sqlx::{PgPool, Row};
use testkit_rs::postgres::{FakePostgres, PgType, Response};

async fn deactivate_user(pool: &PgPool, user_id: i32) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;

    sqlx::query("UPDATE users SET active = false WHERE id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;

    sqlx::query("INSERT INTO audit_log (user_id, action) VALUES ($1, $2)")
        .bind(user_id)
        .bind("user_deactivated")
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await
}

#[tokio::test]
async fn rows_response_returns_mocked_rows() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock(
        "SELECT id, name FROM users WHERE id = $1",
        Response::rows(
            &[("id", PgType::Int4), ("name", PgType::Text)],
            vec![vec![1.into(), "alice".into()]],
        ),
    );
    let pool = pg.pool().await;

    let row = sqlx::query("SELECT id, name FROM users WHERE id = $1")
        .bind(1_i32)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<i32, _>("id"), 1);
    assert_eq!(row.get::<String, _>("name"), "alice");
}

#[tokio::test]
async fn command_response_reports_affected_rows() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock(
        "UPDATE users SET active = false WHERE expired = true",
        Response::command("UPDATE 3"),
    );
    let pool = pg.pool().await;

    let result = sqlx::query("UPDATE users SET active = false WHERE expired = true")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(result.rows_affected(), 3);
}

#[tokio::test]
async fn error_response_returns_database_error() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock(
        "INSERT INTO users (email) VALUES ($1)",
        Response::error("23505", "email already exists"),
    );
    let pool = pg.pool().await;

    let error = sqlx::query("INSERT INTO users (email) VALUES ($1)")
        .bind("alice@example.com")
        .execute(&pool)
        .await
        .unwrap_err();
    let database_error = error.as_database_error().unwrap();

    assert_eq!(database_error.code().as_deref(), Some("23505"));
    assert_eq!(database_error.message(), "email already exists");
}

#[tokio::test]
async fn unmatched_query_succeeds_with_zero_affected_rows() {
    let pg = FakePostgres::start().await.unwrap();
    let pool = pg.pool().await;

    let result = sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(result.rows_affected(), 0);
}

#[tokio::test]
async fn default_error_rejects_unmatched_queries() {
    let pg = FakePostgres::start().await.unwrap();
    pg.set_default(Response::error("XX000", "unexpected query"));
    let pool = pg.pool().await;

    let error = sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap_err();
    let database_error = error.as_database_error().unwrap();

    assert_eq!(database_error.code().as_deref(), Some("XX000"));
    assert_eq!(database_error.message(), "unexpected query");
}

#[tokio::test]
async fn contains_and_regex_rules_match_queries() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock_contains("FROM users WHERE active", Response::command("SELECT 0"))
        .mock_regex(
            r"^DELETE FROM sessions WHERE user_id = \$1$",
            Response::command("DELETE 2"),
        );
    let pool = pg.pool().await;

    let selected = sqlx::query("SELECT id FROM users WHERE active = true")
        .execute(&pool)
        .await
        .unwrap();
    let deleted = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(7_i32)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(selected.rows_affected(), 0);
    assert_eq!(deleted.rows_affected(), 2);
}

#[tokio::test]
async fn queries_records_sql_and_bind_parameters() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock(
        "DELETE FROM users WHERE id = $1",
        Response::command("DELETE 1"),
    );
    let pool = pg.pool().await;

    sqlx::query("DELETE   FROM users WHERE id = $1;")
        .bind(7_i32)
        .execute(&pool)
        .await
        .unwrap();

    let queries = pg.queries();
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].sql, "DELETE FROM users WHERE id = $1");
    assert_eq!(queries[0].params, vec![Some(7_i32.to_be_bytes().to_vec())]);
}

#[tokio::test]
async fn function_executes_multiple_queries_in_a_transaction() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock(
        "UPDATE users SET active = false WHERE id = $1",
        Response::command("UPDATE 1"),
    )
    .mock(
        "INSERT INTO audit_log (user_id, action) VALUES ($1, $2)",
        Response::command("INSERT 0 1"),
    );
    let pool = pg.pool().await;

    deactivate_user(&pool, 7).await.unwrap();

    let queries = pg.queries();
    let sql = queries
        .iter()
        .map(|query| query.sql.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        sql,
        [
            "BEGIN",
            "UPDATE users SET active = false WHERE id = $1",
            "INSERT INTO audit_log (user_id, action) VALUES ($1, $2)",
            "COMMIT",
        ]
    );
    assert_eq!(queries[1].params, vec![Some(7_i32.to_be_bytes().to_vec())]);
    assert_eq!(
        queries[2].params,
        vec![
            Some(7_i32.to_be_bytes().to_vec()),
            Some(b"user_deactivated".to_vec()),
        ]
    );
}

#[tokio::test]
async fn clear_queries_only_clears_recorded_history() {
    let pg = FakePostgres::start().await.unwrap();
    pg.mock("DELETE FROM users", Response::command("DELETE 1"));
    let pool = pg.pool().await;

    sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(pg.queries().len(), 1);

    pg.clear_queries();
    assert!(pg.queries().is_empty());

    let result = sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(result.rows_affected(), 1);
    assert_eq!(pg.queries().len(), 1);
}
