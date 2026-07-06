//! Jest / Vitest failures (spec §16.5). Captures `FAIL <file>` and, when
//! present, the nearest `expected`/`received` detail.

use super::findings::{Finding, Labels};
use regex::Regex;
use std::sync::LazyLock;

// File-level failures (`FAIL <file>`). The per-test `●`/`✕`/`×` markers are
// intentionally excluded so a failing file isn't counted once per test.
static FAIL_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*FAIL\s+(\S+.*?)\s*$").unwrap());
static EXPECTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)expected[: ]+(.+?)\s+received[: ]+(.+)").unwrap());

pub fn looks_like(normalized: &str) -> bool {
    normalized.contains("FAIL ")
        || normalized.contains("Test Files")
        || normalized.contains("Tests:")
}

pub fn parse(normalized: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    // Attach an expected/received detail found on a nearby line.
    let lines: Vec<&str> = normalized.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if let Some(c) = FAIL_FILE.captures(line) {
            let file = c[1].trim().to_string();
            // Skip summary lines like "FAIL  src (2 tests)" noise is fine.
            let detail = lines[i..(i + 8).min(lines.len())]
                .iter()
                .find_map(|l| EXPECTED.captures(l))
                .map(|c| format!("expected {}, received {}", c[1].trim(), c[2].trim()));
            findings.push(Finding {
                file,
                loc: String::new(),
                code: "FAIL".to_string(),
                detail,
            });
        }
    }
    findings
}

pub fn labels() -> Labels {
    Labels {
        noun: "tests",
        fail_status: "tests failed.",
        ok_status: "tests passed.",
        item_plural: "failing tests",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_fail_file_and_expected_received() {
        let text =
            "FAIL tests/auth/session.test.ts\n  ● refresh token\n    expected 403 received 200\n";
        let findings = parse(text);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].file.contains("session.test.ts"));
        assert_eq!(
            findings[0].detail.as_deref(),
            Some("expected 403, received 200")
        );
    }
}
