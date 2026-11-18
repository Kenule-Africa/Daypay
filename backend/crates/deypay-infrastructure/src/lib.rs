//! PostgreSQL repositories and external provider adapters.

pub mod database;
pub mod events;
pub mod repositories;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
