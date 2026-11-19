use sqlx::PgPool;

#[derive(sqlx::FromRow)]
pub struct Document {
    pub document_id: String,
    pub title: String,
    pub version: String,
    pub sha256: String,
}

pub async fn current(pool: &PgPool, scope: &str) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as(
        "SELECT document_id,title,version,sha256 FROM customers.registration_documents
        WHERE scope=$1 AND current ORDER BY document_id",
    )
    .bind(scope)
    .fetch_all(pool)
    .await
}

pub async fn revision(
    pool: &PgPool,
    scope: &str,
    id: &str,
    version: &str,
) -> sqlx::Result<Option<Vec<u8>>> {
    sqlx::query_scalar(
        "SELECT content FROM customers.registration_documents
        WHERE scope=$1 AND document_id=$2 AND version=$3 LIMIT 1",
    )
    .bind(scope)
    .bind(id)
    .bind(version)
    .fetch_optional(pool)
    .await
}
