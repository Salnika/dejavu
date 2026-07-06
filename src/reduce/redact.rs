//! Secret redaction (spec §14). A best-effort safety net, applied *before*
//! anything is written to disk or hashed. Not a guarantee — documented as such.

use regex::Regex;
use std::sync::LazyLock;

/// `(regex, replacement)` pairs. Value-only replacement for `key = value`
/// forms (keeps the key + separator), whole-token replacement for `Bearer`.
static PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(
                r"(?i)\b(AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY|GITHUB_TOKEN|NPM_TOKEN|OPENAI_API_KEY|ANTHROPIC_API_KEY|password|secret|token|api[_-]?key)(\s*[=:]\s*)(\S+)",
            )
            .unwrap(),
            "${1}${2}<REDACTED_SECRET>",
        ),
        (
            Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9\-._~+/]+=*").unwrap(),
            "Bearer <REDACTED_SECRET>",
        ),
    ]
});

/// Redact secrets in a string. Returns the redacted text and whether anything
/// changed.
pub fn redact_str(text: &str) -> (String, bool) {
    let mut out = text.to_string();
    for (re, replacement) in PATTERNS.iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    let changed = out != text;
    (out, changed)
}

/// Redact secrets in raw bytes (decoded lossily as UTF-8, matching how output
/// is later stored and hashed).
pub fn redact_bytes(input: &[u8]) -> (Vec<u8>, bool) {
    let text = String::from_utf8_lossy(input);
    let (redacted, changed) = redact_str(&text);
    (redacted.into_bytes(), changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_named_secrets_keeping_key() {
        let (out, changed) = redact_str("export AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMIabcd1234");
        assert!(changed);
        assert!(out.contains("AWS_SECRET_ACCESS_KEY="));
        assert!(out.contains("<REDACTED_SECRET>"));
        assert!(!out.contains("wJalrXUtnFEMIabcd1234"));
    }

    #[test]
    fn redacts_bearer_and_generic_kv() {
        let (out, _) = redact_str("Authorization: Bearer abc.def.ghi123\ntoken=supersecretvalue");
        assert!(out.contains("Bearer <REDACTED_SECRET>"));
        assert!(!out.contains("abc.def.ghi123"));
        assert!(out.contains("token=<REDACTED_SECRET>"));
        assert!(!out.contains("supersecretvalue"));
    }

    #[test]
    fn leaves_normal_text_untouched() {
        let (out, changed) = redact_str("the tests passed\n3 files changed");
        assert!(!changed);
        assert_eq!(out, "the tests passed\n3 files changed");
    }
}
