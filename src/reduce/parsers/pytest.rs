//! Pytest failures (spec §16.6). Uses the `FAILED <file>::<test>` short-summary
//! lines as the primary signal.

use super::findings::{Finding, Labels};
use regex::Regex;
use std::sync::LazyLock;

static FAILED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^FAILED\s+(\S+?)(?:::(\S+))?(?:\s+-\s+(.*))?$").unwrap());

pub fn looks_like(normalized: &str) -> bool {
    normalized.contains("FAILED ")
        || normalized.contains("=== FAILURES ===")
        || normalized.contains("short test summary")
}

pub fn parse(normalized: &str) -> Vec<Finding> {
    FAILED
        .captures_iter(normalized)
        .map(|c| {
            let file = c.get(1).map(|m| m.as_str()).unwrap_or_default().to_string();
            let test = c.get(2).map(|m| m.as_str()).unwrap_or_default().to_string();
            Finding {
                file: if test.is_empty() {
                    file
                } else {
                    format!("{file}::{test}")
                },
                loc: String::new(),
                code: "FAILED".to_string(),
                detail: c.get(3).map(|m| m.as_str().trim().to_string()),
            }
        })
        .collect()
}

pub fn labels() -> Labels {
    Labels {
        noun: "pytest",
        fail_status: "pytest failed.",
        ok_status: "pytest passed.",
        item_plural: "failing tests",
    }
}
