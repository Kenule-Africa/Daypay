use crate::repositories::storage_error;
use deypay_application::events::{EventWriter, NewAuditEvent, NewOutboxEvent};
use deypay_application::repositories::RepositoryError;
use sqlx::{Postgres, Transaction};

pub struct PgEventWriter<'a, 'db> {
    transaction: &'a mut Transaction<'db, Postgres>,
}

impl<'a, 'db> PgEventWriter<'a, 'db> {
    pub fn new(transaction: &'a mut Transaction<'db, Postgres>) -> Self {
        Self { transaction }
    }
}

impl EventWriter for PgEventWriter<'_, '_> {
    async fn append_outbox(&mut self, event: NewOutboxEvent<'_>) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO integrations.outbox_events (id,aggregate_type,aggregate_id,event_type,payload)
            VALUES ($1,$2,$3,$4,$5)")
            .bind(event.id).bind(event.aggregate_type).bind(event.aggregate_id).bind(event.event_type)
            .bind(sqlx::types::Json(event.payload))
            .execute(&mut **self.transaction).await.map_err(storage_error)?;
        Ok(())
    }

    async fn append_audit(&mut self, event: NewAuditEvent<'_>) -> Result<(), RepositoryError> {
        sqlx::query("INSERT INTO audit.audit_events (id,actor_type,actor_id,action,resource_type,resource_id,reason,correlation_id,metadata)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(event.id).bind(event.actor_type).bind(event.actor_id).bind(event.action)
            .bind(event.resource_type).bind(event.resource_id).bind(event.reason).bind(event.correlation_id)
            .bind(sqlx::types::Json(event.metadata))
            .execute(&mut **self.transaction).await.map_err(storage_error)?;
        Ok(())
    }
}
