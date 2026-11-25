use deypay_application::events::{EventWriter, NewAuditEvent, NewOutboxEvent};
use deypay_application::repositories::RepositoryError;
use deypay_application::repositories::{IdentityRepository, TransactionRepository};
use deypay_infrastructure::MIGRATOR;
use deypay_infrastructure::events::PgEventWriter;
use deypay_infrastructure::repositories::PgRepositories;
use sqlx::PgPool;

#[path = "persistence/profiles.rs"]
mod profiles;

#[sqlx::test(migrator = "MIGRATOR")]
async fn audit_and_outbox_follow_caller_commit_or_rollback(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!(
        "../../../../infrastructure/postgres/phase-two-grants.psql"
    ))
    .execute(&pool)
    .await?;
    let payload = serde_json::json!({});
    let aggregate = Uuid::from_u128(1);
    let outbox = |id| NewOutboxEvent {
        id,
        aggregate_type: "test",
        aggregate_id: aggregate,
        event_type: "test",
        payload: &payload,
    };
    let audit = || NewAuditEvent {
        id: aggregate,
        actor_type: "SYSTEM",
        actor_id: None,
        action: "TEST",
        resource_type: "test",
        resource_id: Some(aggregate),
        reason: None,
        correlation_id: None,
        metadata: &payload,
    };

    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL ROLE deypay_app")
        .execute(&mut *tx)
        .await?;
    let mut writer = PgEventWriter::new(&mut tx);
    writer
        .append_outbox(outbox(Uuid::from_u128(2)))
        .await
        .unwrap();
    writer.append_audit(audit()).await.unwrap();
    tx.commit().await?;

    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL ROLE deypay_app")
        .execute(&mut *tx)
        .await?;
    let mut writer = PgEventWriter::new(&mut tx);
    writer
        .append_outbox(outbox(Uuid::from_u128(3)))
        .await
        .unwrap();
    assert_eq!(
        writer.append_audit(audit()).await,
        Err(RepositoryError::Conflict)
    );
    tx.rollback().await?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM integrations.outbox_events")
        .fetch_all(&pool)
        .await?;
    assert_eq!(ids, vec![Uuid::from_u128(2)]);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_job_claims_skip_locked_rows(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO integrations.jobs(job_type,execution_key,payload,max_attempts,available_at)
        VALUES ('test','one','{}',2,now()),('test','two','{}',2,now())",
    )
    .execute(&pool)
    .await?;
    let claim = "SELECT id FROM integrations.jobs WHERE status='PENDING' AND available_at <= now()
        ORDER BY available_at,id LIMIT 1 FOR UPDATE SKIP LOCKED";
    let mut first = pool.begin().await?;
    let first_id: Uuid = sqlx::query_scalar(claim).fetch_one(&mut *first).await?;
    let mut second = pool.begin().await?;
    let second_id: Uuid = sqlx::query_scalar(claim).fetch_one(&mut *second).await?;
    assert_ne!(first_id, second_id);
    first.rollback().await?;
    second.rollback().await?;
    Ok(())
}
use uuid::Uuid;

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_reads_are_scoped_and_exact(pool: PgPool) -> sqlx::Result<()> {
    let owner: Uuid = sqlx::query_scalar("INSERT INTO identity.users(email,status) VALUES ('repo@example.test','REGISTERED') RETURNING id")
        .fetch_one(&pool).await?;
    let asset: Uuid = sqlx::query_scalar("INSERT INTO wallets.assets(symbol,name,asset_type,accounting_precision) VALUES ('USDC','fixture','TEST',6) RETURNING id")
        .fetch_one(&pool).await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO payments.transactions(user_id,public_reference,transaction_type,status,source_asset_id,source_amount_minor)
        VALUES ($1,'repo-ref','INTERNAL_TRANSFER','CREATED',$2,99999999999999999999999999999999999999) RETURNING id")
        .bind(owner).bind(asset).fetch_one(&pool).await?;
    let repo = PgRepositories::new(pool);
    assert_eq!(
        repo.identity(owner).await.unwrap().unwrap().status,
        "REGISTERED"
    );
    let record = repo.transaction(owner, id).await.unwrap().unwrap();
    let amount = record.source.unwrap();
    assert_eq!(amount.asset_id, asset);
    assert_eq!(amount.units.value(), 10_u128.pow(38) - 1);
    assert!(record.destination.is_none());
    assert!(repo.transaction(Uuid::nil(), id).await.unwrap().is_none());
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn runtime_column_privileges(pool: PgPool) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(include_str!(
        "../../../../infrastructure/postgres/phase-two-grants.psql"
    ))
    .execute(&mut *tx)
    .await?;
    sqlx::raw_sql(include_str!("runtime-role.psql"))
        .execute(&mut *tx)
        .await?;
    tx.rollback().await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn delivery_and_audit(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!("cases/delivery_and_audit.sql"))
        .execute(&pool)
        .await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn money_and_wallets(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!("cases/money_and_wallets.sql"))
        .execute(&pool)
        .await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn identity_constraints(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!("cases/identity_constraints.sql"))
        .execute(&pool)
        .await?;
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn migrations_apply_and_replay(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    MIGRATOR.run(&pool).await?;
    MIGRATOR.run(&pool).await?;
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(&pool)
            .await?;
    assert_eq!(
        versions,
        MIGRATOR.iter().map(|m| m.version).collect::<Vec<_>>()
    );
    Ok(())
}
