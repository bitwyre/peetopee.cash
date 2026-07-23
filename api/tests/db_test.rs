use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn migrations_create_tables(pool: PgPool) {
    for table in ["users", "login_tokens", "sessions", "orders"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
}
