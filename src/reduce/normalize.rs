//! Output normalization (spec §13): strip volatile noise so equivalent runs
//! hash identically, without hiding useful information.

use regex::Regex;
use std::sync::LazyLock;

static ISO_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?").unwrap()
});
static CLOCK_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{2}:\d{2}:\d{2}(?:\.\d+)?\b").unwrap());
static DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d+(?:\.\d+)?\s?(?:ms|µs|us|ns|s|m)\b").unwrap());
static TMP_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:/private/tmp|/tmp|/var/folders)/[^\s:]*").unwrap());
static PID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bpid\s+\d+").unwrap());
static PORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:localhost|127\.0\.0\.1):\d{2,5}\b").unwrap());
static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
/// Package-manager self-update banners: appear on some runs and not others,
/// breaking `unchanged` detection for otherwise-identical output (spec §1
/// "warnings package manager repetes"). Update notices only — never warnings.
static PM_NOTICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(npm notice\b.*|.*update available!?\s.*→.*|\[?yarn\]? .*new version .*available.*)$",
    )
    .unwrap()
});

const MAX_LINE_CHARS: usize = 2000;
const LINE_KEEP: usize = 1000;

/// Normalize captured output. Ordering is load-bearing: ANSI first, then time
/// placeholders before durations.
pub fn normalize(input: &str) -> String {
    // 0. expand tabs to spaces first: the VTE-based ANSI stripper drops tab
    // control chars, which would destroy tab-based indentation (e.g. git status).
    let detabbed = input.replace('\t', "    ");

    // 1. strip ANSI escapes.
    let stripped = strip_ansi_escapes::strip(detabbed.as_bytes());
    let text = String::from_utf8_lossy(&stripped);

    // 2. CRLF / lone CR -> LF.
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    // 5-9. volatile-token placeholders.
    let text = ISO_TIMESTAMP.replace_all(&text, "<TIMESTAMP>");
    let text = CLOCK_TIME.replace_all(&text, "<TIME>");
    let text = DURATION.replace_all(&text, "<DURATION>");
    let text = TMP_PATH.replace_all(&text, "<TMP_PATH>");
    let text = PID.replace_all(&text, "pid <PID>");
    let text = PORT.replace_all(&text, "localhost:<PORT>");

    // 3, 10, 11. line pass: trailing whitespace, drop progress bars, truncate
    // very long lines.
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        if is_progress_bar(line) || PM_NOTICE.is_match(line) {
            continue;
        }
        lines.push(truncate_long_line(line.trim_end()));
    }
    let joined = lines.join("\n");

    // 4. collapse 2+ blank lines into one.
    BLANKS.replace_all(&joined, "\n\n").into_owned()
}

/// A progress-bar / spinner line to drop. Conservative: always drop lines with
/// block-drawing chars; drop ASCII bars only when a percentage is present (so
/// markdown tables and separators survive).
fn is_progress_bar(line: &str) -> bool {
    let t = line.trim();
    if t.chars().count() < 8 {
        return false;
    }
    if t.chars().any(|c| ('\u{2588}'..='\u{258F}').contains(&c)) {
        return true;
    }
    if !t.contains('%') {
        return false;
    }
    let total = t.chars().count();
    let barish = t
        .chars()
        .filter(|c| matches!(c, '=' | '>' | '#' | '-' | '.' | ' ' | '[' | ']'))
        .count();
    barish * 10 >= total * 7
}

fn truncate_long_line(line: &str) -> String {
    if line.chars().count() <= MAX_LINE_CHARS {
        return line.to_string();
    }
    let head: String = line.chars().take(LINE_KEEP).collect();
    let tail_rev: Vec<char> = line.chars().rev().take(LINE_KEEP).collect();
    let tail: String = tail_rev.into_iter().rev().collect();
    format!("{head}…<TRUNCATED>…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_normalizes_newlines() {
        let out = normalize("\x1b[31mred\x1b[0m\r\ntext\r\n");
        assert_eq!(out, "red\ntext\n");
    }

    #[test]
    fn replaces_volatile_tokens() {
        let out =
            normalize("done in 843ms at 2026-07-06T10:41:22.123Z pid 12345 on localhost:5173");
        assert!(out.contains("<DURATION>"));
        assert!(out.contains("<TIMESTAMP>"));
        assert!(out.contains("pid <PID>"));
        assert!(out.contains("localhost:<PORT>"));
        assert!(!out.contains("843ms"));
        assert!(!out.contains("12345"));
    }

    #[test]
    fn tmp_paths_and_clock_times() {
        let out = normalize("wrote /tmp/foo-839201/bar.txt at 12:41:22");
        assert!(out.contains("<TMP_PATH>"));
        assert!(out.contains("<TIME>"));
    }

    #[test]
    fn collapses_blank_lines_and_trims() {
        let out = normalize("a   \n\n\n\nb");
        assert_eq!(out, "a\n\nb");
    }

    #[test]
    fn equivalent_runs_normalize_identically() {
        let a = normalize("PASS in 120ms at 10:00:01");
        let b = normalize("PASS in 456ms at 11:22:33");
        assert_eq!(a, b);
    }

    #[test]
    fn npm_update_notice_is_dropped() {
        let with_notice = "ok\nnpm notice\nnpm notice New minor version of npm available! 11.16.0 -> 11.18.0\nnpm notice To update run: npm install -g npm@11.18.0\ndone";
        let without = "ok\ndone";
        assert_eq!(normalize(with_notice), normalize(without));
        // Warnings survive.
        assert!(normalize("npm warn deprecated foo@1").contains("npm warn"));
    }
}
