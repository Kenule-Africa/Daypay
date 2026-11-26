//! Workstation-only email outbox (IMP-P3-002 and IMP-P3-008 local acceptance).
//!
//! Runs the production `VerificationWorker` and, when `PASSWORD_RESET_BASE_URL`
//! is set, `RecoveryWorker` once each: dispatch, claim, token, prepare, finish.
//! The run uses a local or test database. Instead of contacting an email
//! provider, it writes each email to a private directory. The operator then
//! opens the link with the real `/verify-email` or `/reset-password` page and
//! endpoint.
//!
//! Guards:
//! - `APP_ENV` must be `local` or `test`. Staging and production are refused.
//! - Each link page must be a loopback `/verify-email` or `/reset-password` URL.
//! - The sandbox workers claim only sandbox-scoped customers, so hosted,
//!   published-scope customers are never processed.
//! - Each worker uses a fresh random HMAC key per run that never leaves the
//!   process.
//! - It is a Cargo example: no deployable binary or image contains it.
//!
//! This exercises local token generation and redemption only. It is not
//! evidence of external email delivery.

use deypay_common::{
    config::{Environment, Settings},
    runtime::init_logging,
};
use deypay_infrastructure::{
    database,
    email_delivery::{Delivery, EmailDelivery, Message, SendResult},
    email_verification::VerificationWorker,
    password_recovery::RecoveryWorker,
};
use std::{
    error::Error,
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use url::{Host, Url};
use uuid::Uuid;

/// Upper bound on rounds handled in one run, so a misbehaving queue cannot spin.
const MAX_ROUNDS: usize = 100;
/// Readiness probes (two seconds each, one second apart) before reporting the database unavailable.
const READY_ATTEMPTS: usize = 5;

fn local(environment: Environment) -> bool {
    matches!(environment, Environment::Local | Environment::Test)
}

fn loopback_page(value: &str, path: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let loopback = match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(name)) => name == "localhost",
        None => false,
    };
    loopback && url.path() == path
}

fn random_key() -> Result<[u8; 32], &'static str> {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|_| "Operating-system randomness unavailable")?;
    Ok(key)
}

/// A private, write-once directory standing in for the recipient's mailbox.
struct Outbox {
    dir: PathBuf,
    written: AtomicUsize,
}

impl Outbox {
    fn open(dir: &str) -> Result<Self, &'static str> {
        let dir = PathBuf::from(dir);
        if !dir.is_absolute() || !dir.is_dir() {
            return Err("DEYPAY_LOCAL_OUTBOX_DIR must be an existing absolute directory");
        }
        Ok(Self {
            dir,
            written: AtomicUsize::new(0),
        })
    }

    fn store(&self, id: Uuid, message: &Message) -> Delivery {
        let body = format!(
            "To: {}\nSubject: {}\n\n{}\n",
            message.to[0], message.subject, message.text
        );
        let path = self.dir.join(format!("{id}.txt"));
        match create_private(&path) {
            Ok(mut file) => {
                if file
                    .write_all(body.as_bytes())
                    .and_then(|()| file.sync_all())
                    .is_err()
                {
                    return Delivery::Transient;
                }
                self.written.fetch_add(1, Ordering::Relaxed);
                Delivery::Accepted(None)
            }
            // A reclaimed job carries the identical message; anything else is not ours to overwrite.
            Err(e) if e.kind() == ErrorKind::AlreadyExists => match std::fs::read(&path) {
                Ok(existing) if existing == body.as_bytes() => Delivery::Accepted(None),
                Ok(_) => Delivery::Permanent,
                Err(_) => Delivery::Transient,
            },
            Err(_) => Delivery::Transient,
        }
    }
}

fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    // On Windows the file inherits the directory ACL; the directory must be operator-private.
    options.open(path)
}

impl EmailDelivery for Outbox {
    fn send<'a>(&'a self, id: Uuid, message: &'a Message) -> SendResult<'a> {
        Box::pin(async move { self.store(id, message) })
    }
}

fn required(key: &'static str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("Missing {key}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    if !local(settings.environment) {
        return Err("The local email outbox runs only with APP_ENV=local or test".into());
    }
    init_logging(&settings.log_level)?;
    let page = required("EMAIL_VERIFY_BASE_URL")?;
    if !loopback_page(&page, "/verify-email") {
        return Err("EMAIL_VERIFY_BASE_URL must be a loopback /verify-email page".into());
    }
    let reset_page = std::env::var("PASSWORD_RESET_BASE_URL").ok();
    if reset_page
        .as_deref()
        .is_some_and(|url| !loopback_page(url, "/reset-password"))
    {
        return Err("PASSWORD_RESET_BASE_URL must be a loopback /reset-password page".into());
    }
    let outbox = Arc::new(Outbox::open(&required("DEYPAY_LOCAL_OUTBOX_DIR")?)?);
    let verification = VerificationWorker::sandbox(outbox.clone(), random_key()?, &page)?;
    let recovery = match reset_page {
        Some(url) => Some(RecoveryWorker::sandbox(
            outbox.clone(),
            random_key()?,
            &url,
        )?),
        None => None,
    };
    let pool = database::pool(&settings)?;
    // A busy workstation can exceed the two-second readiness probe on the first
    // SCRAM login, so retry briefly before giving up.
    let mut ready = false;
    for _ in 0..READY_ATTEMPTS {
        if database::ready(&pool).await {
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if !ready {
        return Err("Database unavailable".into());
    }
    let mut rounds = 0;
    while rounds < MAX_ROUNDS {
        let mut busy = verification.tick(&pool).await?;
        if let Some(recovery) = &recovery {
            busy |= recovery.tick(&pool).await?;
        }
        if !busy {
            break;
        }
        rounds += 1;
    }
    pool.close().await;
    println!(
        "Processed {rounds} round(s) of email jobs (password reset {}); wrote {} message(s) to {}",
        if recovery.is_some() { "on" } else { "off" },
        outbox.written.load(Ordering::Relaxed),
        outbox.dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_infrastructure::email_delivery::{password_reset_message, verification_message};

    fn scratch(prefix: &str) -> PathBuf {
        let mut bytes = [0u8; 16];
        getrandom::getrandom(&mut bytes).unwrap();
        std::env::temp_dir().join(format!("{prefix}-{}", Uuid::from_bytes(bytes)))
    }

    #[test]
    fn refuses_hosted_environments() {
        assert!(local(Environment::Local) && local(Environment::Test));
        assert!(!local(Environment::Staging) && !local(Environment::Production));
    }

    #[test]
    fn accepts_only_loopback_link_pages() {
        for (url, path) in [
            ("https://127.0.0.2:8443/verify-email", "/verify-email"),
            ("http://127.0.0.1:8000/verify-email", "/verify-email"),
            ("https://localhost/verify-email", "/verify-email"),
            ("https://[::1]:8443/verify-email", "/verify-email"),
            ("https://127.0.0.2:8443/reset-password", "/reset-password"),
        ] {
            assert!(loopback_page(url, path), "{url}");
        }
        for (url, path) in [
            ("https://app.example.test/verify-email", "/verify-email"),
            ("https://10.0.0.1/verify-email", "/verify-email"),
            (
                "https://127.0.0.1.example.test/verify-email",
                "/verify-email",
            ),
            (
                "https://localhost.example.test/verify-email",
                "/verify-email",
            ),
            ("https://127.0.0.1/elsewhere", "/verify-email"),
            ("https://127.0.0.2:8443/verify-email", "/reset-password"),
            ("https://app.example.test/reset-password", "/reset-password"),
            ("not a url", "/verify-email"),
        ] {
            assert!(!loopback_page(url, path), "{url}");
        }
    }

    #[test]
    fn outbox_requires_an_existing_absolute_directory() {
        assert!(Outbox::open("relative/outbox").is_err());
        assert!(Outbox::open(scratch("deypay-missing").to_str().unwrap()).is_err());
    }

    #[test]
    fn outbox_writes_each_message_once() {
        let dir = scratch("deypay-outbox");
        std::fs::create_dir(&dir).unwrap();
        let outbox = Outbox::open(dir.to_str().unwrap()).unwrap();
        let link = Url::parse("https://127.0.0.2:8443/verify-email#fixture").unwrap();
        let message = verification_message("one@example.test", &link, "2030-05-20 09:15:30");
        let id = Uuid::from_u128(1);
        assert_eq!(outbox.store(id, &message), Delivery::Accepted(None));
        let saved = std::fs::read_to_string(dir.join(format!("{id}.txt"))).unwrap();
        assert!(
            saved.starts_with("To: one@example.test\nSubject: Verify your DEYPAY email address\n")
        );
        assert!(saved.contains(link.as_str()));
        // A reclaimed job with the identical message is idempotent; a changed one is refused.
        assert_eq!(outbox.store(id, &message), Delivery::Accepted(None));
        let other = verification_message("two@example.test", &link, "2030-05-20 09:15:30");
        assert_eq!(outbox.store(id, &other), Delivery::Permanent);
        assert_eq!(outbox.written.load(Ordering::Relaxed), 1);
        let reset = Url::parse("https://127.0.0.2:8443/reset-password#fixture").unwrap();
        let message = password_reset_message("one@example.test", &reset, "2030-05-20 09:15:30");
        let id = Uuid::from_u128(2);
        assert_eq!(outbox.store(id, &message), Delivery::Accepted(None));
        let saved = std::fs::read_to_string(dir.join(format!("{id}.txt"))).unwrap();
        assert!(saved.starts_with("To: one@example.test\nSubject: Reset your DEYPAY password\n"));
        assert!(saved.contains(reset.as_str()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
