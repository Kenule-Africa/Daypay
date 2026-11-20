use crate::config::Environment;

pub fn enabled() -> Result<bool, &'static str> {
    match std::env::var("EMAIL_VERIFICATION_ENABLED").as_deref() {
        Ok("true") => Ok(true),
        Err(std::env::VarError::NotPresent) | Ok("false") => Ok(false),
        _ => Err("Invalid EMAIL_VERIFICATION_ENABLED"),
    }
}

pub fn scope(env: Environment) -> &'static str {
    match env {
        Environment::Local | Environment::Test => "sandbox",
        Environment::Staging | Environment::Production => "published",
    }
}

pub fn number(key: &'static str, default: i32, min: i32, max: i32) -> Result<i32, &'static str> {
    match std::env::var(key) {
        Err(std::env::VarError::NotPresent) => Ok(default),
        Ok(value) => value
            .parse()
            .ok()
            .filter(|n| (min..=max).contains(n))
            .ok_or(key),
        _ => Err(key),
    }
}
