//! Workstation-only email-verification outbox (IMP-P3-002 local acceptance).
//!
//! Runs the production `VerificationWorker` dispatch, claim, token, prepare and
//! finish path once against a local or test database. Instead of contacting an
//! email provider, it writes each verification email to a private directory so
//! the operator can open the link with the real `/verify-email` page and
//! redemption endpoint.
//!
//! Guards:
//! - `APP_ENV` must be `local` or `test`; staging and production are refused.
//! - The confirmation page must be a loopback `/verify-email` URL.
//! - The sandbox worker claims only customers whose legal acceptance is
//!   sandbox-scoped (`customers.email_verification_eligibility`), so hosted,
//!   published-scope customers are never processed.
//! - Each run uses a fresh random HMAC key that never leaves the process.
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

/// Upper bound on jobs handled in one run, so a misbehaving queue cannot spin.
const MAX_JOBS: usize = 100;

fn local(environment: Environment) -> bool {
    matches!(environment, Environment::Local | Environment::Test)
}

fn loopback_page(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let loopback = match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(name)) => name == "localhost",
        None => false,
    };
    loopback && url.path() == "/verify-email"
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
        return Err("The local verification outbox runs only with APP_ENV=local or test".into());
    }
    init_logging(&settings.log_level)?;
    let page = required("EMAIL_VERIFY_BASE_URL")?;
    if !loopback_page(&page) {
        return Err("EMAIL_VERIFY_BASE_URL must be a loopback /verify-email page".into());
    }
    let outbox = Arc::new(Outbox::open(&required("DEYPAY_LOCAL_OUTBOX_DIR")?)?);
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|_| "Operating-system randomness unavailable")?;
    let worker = VerificationWorker::sandbox(outbox.clone(), key, &page)?;
    let pool = database::pool(&settings)?;
    if !database::ready(&pool).await {
        return Err("Database unavailable".into());
    }
    let mut jobs = 0;
    while jobs < MAX_JOBS && worker.tick(&pool).await? {
        jobs += 1;
    }
    pool.close().await;
    println!(
        "Processed {jobs} verification job(s); wrote {} message(s) to {}",
        outbox.written.load(Ordering::Relaxed),
        outbox.dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use deypay_infrastructure::email_delivery::verification_message;

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
    fn accepts_only_loopback_confirmation_pages() {
        for url in [
            "https://127.0.0.2:8443/verify-email",
            "http://127.0.0.1:8000/verify-email",
            "https://localhost/verify-email",
            "https://[::1]:8443/verify-email",
        ] {
            assert!(loopback_page(url), "{url}");
        }
        for url in [
            "https://app.example.test/verify-email",
            "https://10.0.0.1/verify-email",
            "https://127.0.0.1.example.test/verify-email",
            "https://localhost.example.test/verify-email",
            "https://127.0.0.1/elsewhere",
            "not a url",
        ] {
            assert!(!loopback_page(url), "{url}");
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
        std::fs::remove_dir_all(dir).unwrap();
    }
}
