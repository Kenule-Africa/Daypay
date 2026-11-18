//! Minimal persisted projections. Authentication secrets and payloads stay out of read models.
use crate::money::Money;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerIdentity {
    pub id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KycCase {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub id: Uuid,
    pub symbol: String,
    pub accounting_precision: i16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub asset_id: Uuid,
    pub status: String,
    pub provisioning_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperation {
    pub id: Uuid,
    pub transaction_id: Option<Uuid>,
    pub status: String,
    pub authorized_attempt: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookReceipt {
    pub id: Uuid,
    pub signature_verified: bool,
    pub processing_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEvent {
    pub id: Uuid,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableJob {
    pub id: Uuid,
    pub job_type: String,
    pub status: String,
    pub attempts: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub id: Uuid,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionSummary {
    pub id: Uuid,
    pub public_reference: String,
    pub status: String,
    pub source: Option<Money>,
    pub destination: Option<Money>,
    pub fee: Option<Money>,
}
