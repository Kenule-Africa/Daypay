//! Username format approved by the repository owner on 2026-11-25 (IMP-P3-009).
//!
//! A handle uses ASCII letters, digits, underscore and period, is 3–30
//! characters long, and does not start or end with a period or contain `..`.
//! An optional leading `@` is not part of the handle. Uniqueness and
//! reservations compare the ASCII-lowercased handle, which PostgreSQL computes
//! identically in every locale. ASCII-only handles exclude look-alike letters
//! from other scripts, invisible characters and Unicode normalization variants.
//! The database enforces the same rule on new rows.

pub const MIN_LENGTH: usize = 3;
pub const MAX_LENGTH: usize = 30;

/// Returns the handle without its optional `@` when it satisfies the approved format.
pub fn handle(input: &str) -> Option<&str> {
    let handle = input.strip_prefix('@').unwrap_or(input);
    let valid = handle
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
        && (MIN_LENGTH..=MAX_LENGTH).contains(&handle.len())
        && !handle.starts_with('.')
        && !handle.ends_with('.')
        && !handle.contains("..");
    valid.then_some(handle)
}

/// Read-only view of claimed and reserved handles.
pub trait UsernameDirectory: Send + Sync {
    /// Charges the handle's admission budget (IMP-P3-012) before any lookup.
    fn admit(
        &self,
        handle: &str,
    ) -> impl std::future::Future<Output = Result<(), DirectoryError>> + Send;
    /// Whether the handle is neither reserved nor held by an active account.
    fn available(
        &self,
        handle: &str,
    ) -> impl std::future::Future<Output = Result<bool, DirectoryError>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryError {
    /// The admission budget is exhausted.
    Limited,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvailabilityError {
    /// The input does not satisfy the approved handle format.
    InvalidFormat,
    Limited,
    Unavailable,
}

/// Advisory availability (IMP-P3-009, owner decision of 2026-11-25). A reserved
/// handle and a claimed handle are both reported as unavailable. Registration's
/// unique keys stay the authority, so a later registration can still lose a race.
/// The caller charges the deployment budget before parsing the request.
pub async fn availability(
    directory: &impl UsernameDirectory,
    input: &str,
) -> Result<bool, AvailabilityError> {
    let handle = handle(input).ok_or(AvailabilityError::InvalidFormat)?;
    let failed = |error| match error {
        DirectoryError::Limited => AvailabilityError::Limited,
        DirectoryError::Unavailable => AvailabilityError::Unavailable,
    };
    directory.admit(handle).await.map_err(failed)?;
    directory.available(handle).await.map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_approved_format_with_an_optional_at() {
        for (input, expected) in [
            ("daniel", "daniel"),
            ("@rexzy", "rexzy"),
            ("Ada_Okafor", "Ada_Okafor"),
            ("a.b_c.123", "a.b_c.123"),
            ("abc", "abc"),
            (
                "@abcdefghijklmnopqrstuvwxyz0123",
                "abcdefghijklmnopqrstuvwxyz0123",
            ),
        ] {
            assert_eq!(handle(input), Some(expected), "{input}");
        }
    }

    #[test]
    fn rejects_lengths_symbols_periods_and_lookalikes() {
        for input in [
            "",
            "@",
            "ab",
            "@ab",
            "abcdefghijklmnopqrstuvwxyz01234",
            "@@ada",
            "ada@x",
            ".ada",
            "ada.",
            "a..b",
            "ada okafor",
            "ada-okafor",
            "ada\t",
            "mich\u{430}el",
            "rex\u{200b}zy",
            "jose\u{301}",
            "jos\u{e9}",
            "\u{202e}yzxer",
            "\u{ff44}\u{ff41}\u{ff4e}",
            "\u{c0}da",
        ] {
            assert_eq!(handle(input), None, "{input:?}");
        }
        let oversized = "a".repeat(3000);
        assert_eq!(handle(&oversized), None);
    }

    struct Directory {
        admission: Result<(), DirectoryError>,
        lookup: Result<bool, DirectoryError>,
        log: std::sync::Mutex<Vec<String>>,
    }
    impl Directory {
        fn new(
            admission: Result<(), DirectoryError>,
            lookup: Result<bool, DirectoryError>,
        ) -> Self {
            Self {
                admission,
                lookup,
                log: Default::default(),
            }
        }
        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }
    impl UsernameDirectory for Directory {
        async fn admit(&self, handle: &str) -> Result<(), DirectoryError> {
            self.log.lock().unwrap().push(format!("admit {handle}"));
            self.admission
        }
        async fn available(&self, handle: &str) -> Result<bool, DirectoryError> {
            self.log.lock().unwrap().push(format!("lookup {handle}"));
            self.lookup
        }
    }

    fn run<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, Poll, Waker};
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test doubles complete synchronously"),
        }
    }

    #[test]
    fn availability_validates_then_charges_then_looks_up_the_handle() {
        let directory = Directory::new(Ok(()), Ok(true));
        for invalid in ["ab", "ada-okafor", "mich\u{430}el", "@@ada", ""] {
            assert_eq!(
                run(availability(&directory, invalid)),
                Err(AvailabilityError::InvalidFormat)
            );
        }
        assert!(directory.log().is_empty());
        assert_eq!(run(availability(&directory, "@Ada_Okafor")), Ok(true));
        assert_eq!(directory.log(), ["admit Ada_Okafor", "lookup Ada_Okafor"]);
    }

    #[test]
    fn admission_failures_stop_before_the_lookup() {
        use DirectoryError::{Limited, Unavailable};
        for (admission, lookup, expected) in [
            (Err(Limited), Ok(true), Err(AvailabilityError::Limited)),
            (
                Err(Unavailable),
                Ok(true),
                Err(AvailabilityError::Unavailable),
            ),
            (Ok(()), Ok(false), Ok(false)),
            (
                Ok(()),
                Err(Unavailable),
                Err(AvailabilityError::Unavailable),
            ),
        ] {
            let directory = Directory::new(admission, lookup);
            assert_eq!(run(availability(&directory, "daniel")), expected);
            let lookups = if admission.is_ok() { 2 } else { 1 };
            assert_eq!(directory.log().len(), lookups);
        }
    }
}
