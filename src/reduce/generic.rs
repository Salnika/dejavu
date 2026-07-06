//! Generic validation reducer (spec §16.2): extract priority lines (errors,
//! failures, warnings) with surrounding context. The fallback for every family.

use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

static PRIORITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)error|fail|assertionerror|expected|received|\bTS\d{3,}\b|eslint|warning|panic|exception|traceback",
    )
    .unwrap()
});

/// Per-case success noise: a PASS/ok line is never a priority hit (it may
/// still contain words like `expected=`), and is dropped from bodies — the
/// useful signal of a passing run is its trailing summary, not 100 PASS lines.
static SUCCESS_NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(PASS\b|✓|✔|√|·|ok\b|\d+\s+passing\b)").unwrap());

const CONTEXT_BEFORE: usize = 2;
const CONTEXT_AFTER: usize = 4;
/// When there are no priority hits, how many trailing lines to show
/// (summaries live at the tail of almost every tool's output).
const TAIL_FALLBACK_FAIL: usize = 40;
const TAIL_FALLBACK_PASS: usize = 6;

/// Extract a compact body from normalized output, capped at `max_lines`.
/// `exit_code` steers the shape: passing runs keep only the trailing summary.
pub fn extract(normalized: &str, max_lines: usize, exit_code: i32) -> String {
    let lines: Vec<&str> = normalized.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let noise: Vec<bool> = lines
        .iter()
        .map(|line| SUCCESS_NOISE.is_match(line))
        .collect();
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(i, line)| !noise[*i] && PRIORITY.is_match(line))
        .map(|(i, _)| i)
        .collect();

    // Index set to keep: context windows around hits, plus — for passing runs
    // or hit-less output — the tail summary. A single set dedupes overlap.
    let mut keep: BTreeSet<usize> = BTreeSet::new();
    for &i in &hits {
        let start = i.saturating_sub(CONTEXT_BEFORE);
        let end = (i + CONTEXT_AFTER).min(lines.len() - 1);
        keep.extend((start..=end).filter(|&idx| !noise[idx]));
    }
    if hits.is_empty() || exit_code == 0 {
        let tail_n = if exit_code == 0 {
            TAIL_FALLBACK_PASS
        } else {
            TAIL_FALLBACK_FAIL
        }
        .min(max_lines);
        let mut informative: Vec<usize> = (0..lines.len())
            .filter(|&i| !noise[i] && !lines[i].trim().is_empty())
            .collect();
        if informative.is_empty() {
            // Everything was noise: fall back to the raw tail, never empty.
            informative = (0..lines.len())
                .filter(|&i| !lines[i].trim().is_empty())
                .collect();
        }
        let start = informative.len().saturating_sub(tail_n);
        keep.extend(&informative[start..]);
    }

    let mut out: Vec<String> = Vec::new();
    let mut prev: Option<usize> = None;
    for &idx in &keep {
        if let Some(p) = prev {
            if idx > p + 1 {
                out.push("  ⋮".to_string()); // gap marker
            }
        }
        out.push(lines[idx].to_string());
        prev = Some(idx);
        if out.len() >= max_lines {
            out.push("... (more lines suppressed)".to_string());
            break;
        }
    }
    out.join("\n")
}

/// A short summary for storage (replayed on an `unchanged` run).
pub fn summarize(normalized: &str, exit_code: i32) -> Option<String> {
    let body = extract(normalized, 12, exit_code);
    if body.trim().is_empty() {
        None
    } else {
        Some(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_error_with_context() {
        let text = "setup\nline1\nline2\nError: boom happened\nafter1\nafter2\ntail\nmore\nmore2";
        let body = extract(text, 160, 1);
        assert!(body.contains("Error: boom happened"));
        assert!(body.contains("line2")); // context before
        assert!(body.contains("after1")); // context after
    }

    #[test]
    fn no_hits_returns_tail() {
        let text = "all good\nline 2\nline 3";
        let body = extract(text, 160, 0);
        assert_eq!(body, "all good\nline 2\nline 3");
    }

    #[test]
    fn respects_max_lines() {
        let text: String = (0..200).map(|i| format!("error {i}\n")).collect();
        let body = extract(&text, 20, 1);
        assert!(body.lines().count() <= 21);
    }

    #[test]
    fn pass_lines_are_never_hits_even_with_expected() {
        let mut text: String = (0..100)
            .map(|i| format!("PASS  #{i:03} case expected=$9.99 got=$9.99\n"))
            .collect();
        text.push_str("FAIL  #101 case expected=$6.00 got=$18.00\nTests: 101 total, 1 failed\n");
        let body = extract(&text, 160, 1);
        assert!(body.contains("FAIL  #101"));
        assert!(body.contains("1 failed"));
        // The 100 PASS lines are filtered out entirely.
        assert!(!body.contains("PASS"));
        assert!(body.lines().count() < 12);
    }

    #[test]
    fn passing_run_keeps_only_tail_summary() {
        let mut text: String = (0..120)
            .map(|i| format!("PASS  #{i:03} case expected=$9.99 got=$9.99\n"))
            .collect();
        text.push_str("Tests: 120 total, 120 passed, 0 failed\nAll tests passed.\n");
        let body = extract(&text, 160, 0);
        assert!(body.contains("120 passed"));
        assert!(!body.contains("PASS  #"));
        assert!(body.lines().count() <= TAIL_FALLBACK_PASS + 1);
    }

    #[test]
    fn all_noise_falls_back_to_raw_tail() {
        let text = "PASS a\nPASS b\nPASS c";
        let body = extract(text, 160, 0);
        assert!(!body.is_empty());
    }
}
