//! Event writes join the caller's business transaction.
use crate::repositories::RepositoryError;
use serde_json::Value;
use std::future::Future;
use uuid::Uuid;

pub struct NewOutboxEvent<'a> {
    pub id: Uuid,
    pub aggregate_type: &'a str,
    pub aggregate_id: Uuid,
    pub event_type: &'a str,
    pub payload: &'a Value,
}

pub struct NewAuditEvent<'a> {
    pub id: Uuid,
    pub actor_type: &'a str,
    pub actor_id: Option<Uuid>,
    pub action: &'a str,
    pub resource_type: &'a str,
    pub resource_id: Option<Uuid>,
    pub reason: Option<&'a str>,
    pub correlation_id: Option<Uuid>,
    pub metadata: &'a Value,
}

pub trait EventWriter {
    fn append_outbox(
        &mut self,
        event: NewOutboxEvent<'_>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
    fn append_audit(
        &mut self,
        event: NewAuditEvent<'_>,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;
}
