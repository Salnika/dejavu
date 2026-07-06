//! Small shared helpers.

use chrono::{SecondsFormat, Utc};
use sha2::{Digest, Sha256};

/// Current UTC time as RFC3339 with a `Z` offset and microsecond precision.
/// Lexicographically sortable, which the `created_at < ?` prior-run query needs.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// A fresh run/session id: a hyphenless 32-hex-char UUID v4.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Short display form of a run id (first 12 chars).
pub fn short_id(id: &str) -> &str {
    &id[..id.len().min(12)]
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Estimated token count for a string: `ceil(chars / 4)` (spec §19.1).
pub fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    chars.div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_ceil_of_chars_div_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        // Counts chars, not bytes: "é" is one char.
        assert_eq!(estimate_tokens("é"), 1);
    }

    #[test]
    fn sha256_hex_is_64_chars() {
        assert_eq!(sha256_hex(b"").len(), 64);
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
