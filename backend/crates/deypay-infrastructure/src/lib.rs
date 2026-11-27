//! PostgreSQL repositories and external provider adapters.

pub mod database;
pub mod email_delivery;
pub mod email_verification;
pub mod events;
pub mod kyc_provider_mock;
pub mod ledger;
pub mod legal;
pub mod password_hashing;
pub mod password_recovery;
pub mod password_screening;
pub mod profile_images;
pub mod profiles;
pub mod rate_limit;
pub mod registration;
pub mod repositories;
pub mod sessions;
pub mod transfers;
pub mod wallets;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
