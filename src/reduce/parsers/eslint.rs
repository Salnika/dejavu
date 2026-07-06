//! ESLint diagnostics (spec §16.4). Stateful: a bare file-path line sets the
//! current file; indented `L:C  level  message  rule` lines attach to it.

use super::findings::{Finding, Labels};
use regex::Regex;
use std::sync::LazyLock;

static PROBLEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.*?)\s{2,}(\S+)\s*$").unwrap()
});
static FILE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(/|\./|[A-Za-z0-9_.-]+/)\S*$").unwrap());

pub fn looks_like(normalized: &str) -> bool {
    normalized.lines().any(|l| PROBLEM.is_match(l))
}

pub fn parse(normalized: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut current_file = String::new();
    for line in normalized.lines() {
        if let Some(c) = PROBLEM.captures(line) {
            findings.push(Finding {
                file: current_file.clone(),
                loc: format!("{}:{}", &c[1], &c[2]),
                code: c[5].to_string(),
                detail: Some(format!("{} {}", &c[3], c[4].trim())),
            });
        } else if FILE_LINE.is_match(line.trim_end()) && !line.starts_with(' ') {
            current_file = line.trim().to_string();
        }
    }
    findings
}

pub fn labels() -> Labels {
    Labels {
        noun: "eslint",
        fail_status: "eslint failed.",
        ok_status: "eslint passed.",
        item_plural: "problems",
    }
}
