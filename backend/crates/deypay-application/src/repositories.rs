//! Persistence contracts owned by the application. Customer reads require an owner ID.
use deypay_domain::entities::*;
use std::future::Future;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryError {
    Conflict,
    StorageUnavailable,
    InvalidRecord,
}

pub type Lookup<T> = Result<Option<T>, RepositoryError>;

pub trait IdentityRepository {
    fn identity(&self, customer_id: Uuid) -> impl Future<Output = Lookup<CustomerIdentity>> + Send;
    fn session(
        &self,
        customer_id: Uuid,
        session_id: Uuid,
    ) -> impl Future<Output = Lookup<SessionSummary>> + Send;
}

pub trait KycRepository {
    fn kyc_case(
        &self,
        customer_id: Uuid,
        case_id: Uuid,
    ) -> impl Future<Output = Lookup<KycCase>> + Send;
}

pub trait WalletRepository {
    fn asset(&self, asset_id: Uuid) -> impl Future<Output = Lookup<Asset>> + Send;
    fn wallet(
        &self,
        customer_id: Uuid,
        wallet_id: Uuid,
    ) -> impl Future<Output = Lookup<WalletProfile>> + Send;
}

pub trait ProviderRepository {
    fn provider_operation(
        &self,
        provider_id: Uuid,
        operation_id: Uuid,
    ) -> impl Future<Output = Lookup<ProviderOperation>> + Send;
    fn webhook(
        &self,
        provider_id: Uuid,
        event_id: Uuid,
    ) -> impl Future<Output = Lookup<WebhookReceipt>> + Send;
}

pub trait DeliveryRepository {
    fn outbox_event(&self, event_id: Uuid) -> impl Future<Output = Lookup<OutboxEvent>> + Send;
    fn job(&self, job_id: Uuid) -> impl Future<Output = Lookup<DurableJob>> + Send;
}

pub trait AuditRepository {
    fn audit_event(&self, event_id: Uuid) -> impl Future<Output = Lookup<AuditEvent>> + Send;
}

pub trait TransactionRepository {
    fn transaction(
        &self,
        customer_id: Uuid,
        transaction_id: Uuid,
    ) -> impl Future<Output = Lookup<TransactionSummary>> + Send;
}
