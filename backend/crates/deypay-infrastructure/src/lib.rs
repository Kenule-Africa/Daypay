//! PostgreSQL repositories and external provider adapters.

pub mod database;
pub mod events;
pub mod legal;
pub mod password_hashing;
pub mod password_screening;
pub mod registration;
pub mod repositories;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
