//! Shared helpers for the Phase 4 PostgreSQL cases.
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const GRANTS: [&str; 2] = [
    include_str!("../../../../../infrastructure/postgres/phase-two-grants.psql"),
    include_str!("../../../../../infrastructure/postgres/kyc-grants.psql"),
];

/// Applies the reviewed grants, then returns a pool whose connections act as
/// the restricted runtime role.
pub async fn runtime(pool: &PgPool) -> sqlx::Result<PgPool> {
    for grants in GRANTS {
        sqlx::raw_sql(grants).execute(pool).await?;
    }
    PgPoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}

/// Inserts a customer with `status`, as the schema owner, satisfying the
/// status-dependent identity constraints.
pub async fn customer(pool: &PgPool, email: &str, status: &str) -> sqlx::Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO identity.users(email,status,email_verified_at,closed_at)
        VALUES ($1,$2,CASE WHEN $2 = 'CONTACT_VERIFIED' THEN now() END,
            CASE WHEN $2 = 'CLOSED' THEN now() END) RETURNING id",
    )
    .bind(email)
    .bind(status)
    .fetch_one(pool)
    .await
}

pub fn sql_state(error: &sqlx::Error) -> Option<String> {
    error
        .as_database_error()
        .and_then(|db| db.code())
        .map(|code| code.into_owned())
}
