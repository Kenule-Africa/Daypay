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
}
