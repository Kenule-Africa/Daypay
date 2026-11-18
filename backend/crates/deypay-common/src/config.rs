//! Configuration shared by the API and the worker.

use std::net::SocketAddr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Test,
    Staging,
    Production,
}

// Keep credential-bearing settings out of Debug and log output.
pub struct Settings {
    pub environment: Environment,
    pub database_url: String,
    pub max_connections: u32,
    pub log_level: String,
}

impl Settings {
    pub fn from_env() -> Result<Self, &'static str> {
        Self::read(|key| std::env::var(key).ok())
    }

    fn read(get: impl Fn(&str) -> Option<String>) -> Result<Self, &'static str> {
        let environment = match get("APP_ENV").as_deref() {
            Some("local") => Environment::Local,
            Some("test") => Environment::Test,
            Some("staging") => Environment::Staging,
            Some("production") => Environment::Production,
            _ => return Err("APP_ENV must be local, test, staging or production"),
        };
        let database_url = get("DATABASE_URL")
            .filter(|s| s.starts_with("postgresql://") || s.starts_with("postgres://"))
            .ok_or("DATABASE_URL must be a PostgreSQL URL")?;
        let max_connections = get("DB_MAX_CONNECTIONS")
            .as_deref()
            .unwrap_or("5")
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=100).contains(n))
            .ok_or("DB_MAX_CONNECTIONS must be between 1 and 100")?;
        let log_level = get("LOG_LEVEL").unwrap_or_else(|| String::from("info"));
        if !matches!(log_level.as_str(), "error" | "warn" | "info" | "debug") {
            return Err("LOG_LEVEL must be error, warn, info or debug");
        }
        Ok(Self {
            environment,
            database_url,
            max_connections,
            log_level,
        })
    }
}

pub fn api_bind(value: Option<&str>) -> Result<SocketAddr, &'static str> {
    value
        .unwrap_or("127.0.0.1:8000")
        .parse()
        .map_err(|_| "Invalid API_BIND_ADDR")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(overrides: &[(&str, &str)]) -> Result<Settings, &'static str> {
        let mut values = std::collections::HashMap::from([
            ("APP_ENV", "local"),
            ("DATABASE_URL", "postgres://localhost/deypay"),
        ]);
        values.extend(overrides.iter().copied());
        Settings::read(|key| values.get(key).map(|v| v.to_string()))
    }

    #[test]
    fn limits_and_defaults() {
        assert_eq!(settings(&[]).unwrap().max_connections, 5);
        assert!(api_bind(None).unwrap().ip().is_loopback());
        assert!(api_bind(Some("invalid")).is_err());
        for value in ["0", "101", "-1", "bad"] {
            assert!(settings(&[("DB_MAX_CONNECTIONS", value)]).is_err());
        }
    }

    #[test]
    fn invalid_configuration_fails_without_echoing_secrets() {
        assert!(settings(&[("APP_ENV", "prodution")]).is_err());
        assert!(settings(&[("LOG_LEVEL", "all")]).is_err());
        assert_eq!(
            settings(&[("DATABASE_URL", "private-value")]).err(),
            Some("DATABASE_URL must be a PostgreSQL URL")
        );
    }
}
