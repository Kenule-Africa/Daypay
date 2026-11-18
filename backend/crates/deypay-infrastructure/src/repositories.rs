use deypay_application::repositories::*;
use deypay_domain::{entities::*, money::Money, units::MinorUnits};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgRepositories {
    pool: PgPool,
}
impl PgRepositories {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub(crate) fn storage_error(error: sqlx::Error) -> RepositoryError {
    match error {
        sqlx::Error::Database(ref db) if db.is_unique_violation() => RepositoryError::Conflict,
        sqlx::Error::Database(ref db)
            if db.is_foreign_key_violation() || db.is_check_violation() =>
        {
            RepositoryError::InvalidRecord
        }
        sqlx::Error::ColumnDecode { .. }
        | sqlx::Error::Decode(_)
        | sqlx::Error::ColumnNotFound(_) => RepositoryError::InvalidRecord,
        _ => RepositoryError::StorageUnavailable,
    }
}

impl IdentityRepository for PgRepositories {
    async fn identity(&self, customer_id: Uuid) -> Lookup<CustomerIdentity> {
        let row = sqlx::query("SELECT id, status FROM identity.users WHERE id = $1")
            .bind(customer_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;
        row.map(|row| {
            Ok(CustomerIdentity {
                id: row.try_get("id").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
            })
        })
        .transpose()
    }
    async fn session(&self, customer_id: Uuid, session_id: Uuid) -> Lookup<SessionSummary> {
        let row = sqlx::query("SELECT id, user_id, revoked_at IS NOT NULL AS revoked FROM identity.sessions WHERE user_id = $1 AND id = $2")
            .bind(customer_id)
            .bind(session_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(SessionSummary {
                id: row.try_get("id").map_err(storage_error)?,
                user_id: row.try_get("user_id").map_err(storage_error)?,
                revoked: row.try_get("revoked").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl KycRepository for PgRepositories {
    async fn kyc_case(&self, customer_id: Uuid, case_id: Uuid) -> Lookup<KycCase> {
        let row = sqlx::query(
            "SELECT id, user_id, status FROM compliance.kyc_cases WHERE user_id = $1 AND id = $2",
        )
        .bind(customer_id)
        .bind(case_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| {
            Ok(KycCase {
                id: row.try_get("id").map_err(storage_error)?,
                user_id: row.try_get("user_id").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl WalletRepository for PgRepositories {
    async fn asset(&self, asset_id: Uuid) -> Lookup<Asset> {
        let row = sqlx::query(
            "SELECT id, symbol, accounting_precision, enabled FROM wallets.assets WHERE id = $1",
        )
        .bind(asset_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| {
            Ok(Asset {
                id: row.try_get("id").map_err(storage_error)?,
                symbol: row.try_get("symbol").map_err(storage_error)?,
                accounting_precision: row.try_get("accounting_precision").map_err(storage_error)?,
                enabled: row.try_get("enabled").map_err(storage_error)?,
            })
        })
        .transpose()
    }
    async fn wallet(&self, customer_id: Uuid, wallet_id: Uuid) -> Lookup<WalletProfile> {
        let row = sqlx::query("SELECT id, user_id, asset_id, status, provisioning_status FROM wallets.wallet_profiles WHERE user_id = $1 AND id = $2")
            .bind(customer_id)
            .bind(wallet_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(WalletProfile {
                id: row.try_get("id").map_err(storage_error)?,
                user_id: row.try_get("user_id").map_err(storage_error)?,
                asset_id: row.try_get("asset_id").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
                provisioning_status: row.try_get("provisioning_status").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl ProviderRepository for PgRepositories {
    async fn provider_operation(
        &self,
        provider_id: Uuid,
        operation_id: Uuid,
    ) -> Lookup<ProviderOperation> {
        let row = sqlx::query("SELECT id, transaction_id, status, authorized_attempt FROM integrations.provider_operations WHERE provider_id = $1 AND id = $2")
            .bind(provider_id)
            .bind(operation_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(ProviderOperation {
                id: row.try_get("id").map_err(storage_error)?,
                transaction_id: row.try_get("transaction_id").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
                authorized_attempt: row.try_get("authorized_attempt").map_err(storage_error)?,
            })
        })
        .transpose()
    }
    async fn webhook(&self, provider_id: Uuid, event_id: Uuid) -> Lookup<WebhookReceipt> {
        let row = sqlx::query("SELECT id, signature_verified, processing_status FROM integrations.webhook_events WHERE provider_id = $1 AND id = $2")
            .bind(provider_id)
            .bind(event_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(WebhookReceipt {
                id: row.try_get("id").map_err(storage_error)?,
                signature_verified: row.try_get("signature_verified").map_err(storage_error)?,
                processing_status: row.try_get("processing_status").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl DeliveryRepository for PgRepositories {
    async fn outbox_event(&self, event_id: Uuid) -> Lookup<OutboxEvent> {
        let row = sqlx::query("SELECT id, aggregate_id, event_type, status FROM integrations.outbox_events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(OutboxEvent {
                id: row.try_get("id").map_err(storage_error)?,
                aggregate_id: row.try_get("aggregate_id").map_err(storage_error)?,
                event_type: row.try_get("event_type").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
            })
        })
        .transpose()
    }
    async fn job(&self, job_id: Uuid) -> Lookup<DurableJob> {
        let row = sqlx::query(
            "SELECT id, job_type, status, attempts FROM integrations.jobs WHERE id = $1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| {
            Ok(DurableJob {
                id: row.try_get("id").map_err(storage_error)?,
                job_type: row.try_get("job_type").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
                attempts: row.try_get("attempts").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl AuditRepository for PgRepositories {
    async fn audit_event(&self, event_id: Uuid) -> Lookup<AuditEvent> {
        let row = sqlx::query(
            "SELECT id, action, resource_type, resource_id FROM audit.audit_events WHERE id = $1",
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_error)?;
        row.map(|row| {
            Ok(AuditEvent {
                id: row.try_get("id").map_err(storage_error)?,
                action: row.try_get("action").map_err(storage_error)?,
                resource_type: row.try_get("resource_type").map_err(storage_error)?,
                resource_id: row.try_get("resource_id").map_err(storage_error)?,
            })
        })
        .transpose()
    }
}

impl TransactionRepository for PgRepositories {
    async fn transaction(
        &self,
        customer_id: Uuid,
        transaction_id: Uuid,
    ) -> Lookup<TransactionSummary> {
        let row = sqlx::query("SELECT id, public_reference, status::text AS status, source_asset_id, trunc(source_amount_minor)::text AS source_units, destination_asset_id, trunc(destination_amount_minor)::text AS destination_units, fee_asset_id, trunc(fee_amount_minor)::text AS fee_units FROM payments.transactions WHERE user_id = $1 AND id = $2")
            .bind(customer_id)
            .bind(transaction_id)
            .fetch_optional(&self.pool).await.map_err(storage_error)?;
        row.map(|row| {
            Ok(TransactionSummary {
                id: row.try_get("id").map_err(storage_error)?,
                public_reference: row.try_get("public_reference").map_err(storage_error)?,
                status: row.try_get("status").map_err(storage_error)?,
                source: money(
                    row.try_get("source_asset_id").map_err(storage_error)?,
                    row.try_get("source_units").map_err(storage_error)?,
                )?,
                destination: money(
                    row.try_get("destination_asset_id").map_err(storage_error)?,
                    row.try_get("destination_units").map_err(storage_error)?,
                )?,
                fee: money(
                    row.try_get("fee_asset_id").map_err(storage_error)?,
                    row.try_get("fee_units").map_err(storage_error)?,
                )?,
            })
        })
        .transpose()
    }
}

fn money(asset_id: Option<Uuid>, raw: Option<String>) -> Result<Option<Money>, RepositoryError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let asset_id = asset_id.ok_or(RepositoryError::InvalidRecord)?;
    let units = raw
        .parse::<u128>()
        .ok()
        .and_then(MinorUnits::new)
        .ok_or(RepositoryError::InvalidRecord)?;
    Ok(Some(Money { asset_id, units }))
}
