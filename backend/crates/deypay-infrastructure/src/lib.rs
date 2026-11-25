//! PostgreSQL repositories and external provider adapters.

pub mod database;
pub mod email_delivery;
pub mod email_verification;
pub mod events;
pub mod kyc;
pub mod kyc_provider_mock;
pub mod legal;
pub mod password_hashing;
pub mod password_screening;
pub mod registration;
pub mod repositories;
pub mod restrictions;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
