//! Width-aware render helpers for human-facing command output.
//!
//! Layout adapts to the terminal: tables fit naturally when they can, the last
//! column wraps with a hanging indent when they cannot, and path-heavy lists
//! use one record per entry instead of an unbounded-width column. When stdout
//! is not a terminal (pipes, tests), a stable width of 100 is used.
//!
//! Styling: ANSI colors are applied AFTER layout (never inside width math),
//! and only when stdout is a terminal, `NO_COLOR` is unset, and `TERM` is not
//! `dumb` — piped output stays plain.

use std::io::IsTerminal;
use std::sync::OnceLock;

const FALLBACK_WIDTH: usize = 100;
const MIN_WIDTH: usize = 50;
const MAX_WIDTH: usize = 200;
const GAP: usize = 2;

/// Effective output width, detected once.
pub fn width() -> usize {
    static W: OnceLock<usize> = OnceLock::new();
    *W.get_or_init(|| {
        if std::io::stdout().is_terminal() {
            terminal_size::terminal_size()
                .map(|(w, _)| (w.0 as usize).clamp(MIN_WIDTH, MAX_WIDTH))
                .unwrap_or(FALLBACK_WIDTH)
        } else {
            FALLBACK_WIDTH
        }
    })
}

/// Text styles, resolved to ANSI codes only when color is enabled.
#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Plain,
    Bold,
    Dim,
    Cyan,
    Green,
    Yellow,
    Red,
    BoldGreen,
    BoldCyan,
    /// Semantic: colors by the cell's own text (`ok`/`warn`/`fail`, `active`/`disabled`).
    Status,
}

fn color_enabled() -> bool {
    static C: OnceLock<bool> = OnceLock::new();
    *C.get_or_init(|| {
        std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && std::env::var_os("TERM").is_none_or(|t| t != "dumb")
    })
}

/// Paint `s` with `style` (no-op when color is disabled).
pub fn paint(style: Style, s: &str) -> String {
    if !color_enabled() || style == Style::Plain {
        return s.to_string();
    }
    let code = match style {
        Style::Plain => unreachable!(),
        Style::Bold => "1",
        Style::Dim => "2",
        Style::Cyan => "36",
        Style::Green => "32",
        Style::Yellow => "33",
        Style::Red => "31",
        Style::BoldGreen => "1;32",
        Style::BoldCyan => "1;36",
        Style::Status => {
            return match s.trim() {
                "ok" | "active" => paint(Style::Green, s),
                "warn" | "disabled" => paint(Style::Yellow, s),
                "fail" | "error" => paint(Style::Red, s),
                _ => s.to_string(),
            }
        }
    };
    format!("\x1b[{code}m{s}\x1b[0m")
}

pub fn title(text: &str) {
    println!("{}", paint(Style::BoldGreen, text));
    println!("{}", paint(Style::Green, &"=".repeat(text.chars().count())));
}

pub fn section(text: &str) {
    println!();
    println!("{}", paint(Style::BoldCyan, text));
    println!("{}", paint(Style::Dim, &"-".repeat(text.chars().count())));
}

/// A labeled progress meter, colored by value: `label  ████░░░░ 42.0%`.
/// Higher is better (reduction, savings).
pub fn meter(label: &str, pct: f64) -> String {
    const CELLS: usize = 20;
    let clamped = pct.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * CELLS as f64).round() as usize;
    let style = if clamped >= 60.0 {
        Style::Green
    } else if clamped >= 30.0 {
        Style::Yellow
    } else {
        Style::Red
    };
    let bar = format!(
        "{}{}",
        paint(style, &"█".repeat(filled)),
        paint(Style::Dim, &"░".repeat(CELLS - filled))
    );
    format!("{label}  {bar} {}", paint(style, &format!("{clamped:.1}%")))
}

/// Middle-ellipsis for one-line fields. Path-friendly: keeps more of the tail
/// (the end of a path is usually the discriminating part).
pub fn truncate_middle(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max || max < 8 {
        return s.to_string();
    }
    let keep = max - 1;
    let head = keep / 3;
    let tail = keep - head;
    let head_s: String = s.chars().take(head).collect();
    let tail_s: String = s.chars().skip(n - tail).collect();
    format!("{head_s}…{tail_s}")
}

/// Greedy word-wrap to `max` columns; words longer than a line (paths, hashes)
/// are hard-broken rather than overflowing.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let max = max.max(8);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for word in text.split_whitespace() {
        let mut word_chars = word.chars().count();
        let mut word = word.to_string();
        // Hard-break oversized words.
        while word_chars > max {
            if current_len > 0 {
                lines.push(std::mem::take(&mut current));
                current_len = 0;
            }
            let piece: String = word.chars().take(max).collect();
            word = word.chars().skip(max).collect();
            word_chars -= max;
            lines.push(piece);
        }
        let needed = if current_len == 0 {
            word_chars
        } else {
            current_len + 1 + word_chars
        };
        if needed > max && current_len > 0 {
            lines.push(std::mem::take(&mut current));
            current.push_str(&word);
            current_len = word_chars;
        } else {
            if current_len > 0 {
                current.push(' ');
            }
            current.push_str(&word);
            current_len = needed;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Aligned key–value block; long values wrap with a hanging indent.
pub fn kv(rows: &[(&str, String)]) {
    let key_w = rows
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let value_w = width().saturating_sub(key_w + GAP).max(16);
    for (key, value) in rows {
        let lines = wrap(value, value_w);
        let padded = format!("{key:<key_w$}");
        println!(
            "{}{}{}",
            paint(Style::Dim, &padded),
            " ".repeat(GAP),
            lines[0]
        );
        for line in &lines[1..] {
            println!("{}{line}", " ".repeat(key_w + GAP));
        }
    }
}

/// One block per entry, for lists whose main field is a path or another
/// unbounded string: a colored status bullet + bold main line (middle-truncated
/// when needed), details `·`-joined underneath, dimmed, wrapping with an indent.
pub fn record(bullet: Style, main: &str, details: &[String]) {
    let w = width();
    println!(
        "{} {}",
        paint(bullet, "●"),
        paint(Style::Bold, &truncate_middle(main, w.saturating_sub(2)))
    );
    let detail = details.join(" · ");
    if detail.is_empty() {
        return;
    }
    for line in wrap(&detail, w.saturating_sub(GAP)) {
        println!("{}{}", " ".repeat(GAP), paint(Style::Dim, &line));
    }
}

/// Column-aligned table. Fits naturally when it can; otherwise the LAST column
/// becomes flexible and wraps with a hanging indent. Never overflows `width()`.
pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    table_styled(headers, rows, &[]);
}

/// `table` with a per-column style, applied to every fragment after layout
/// (missing columns fall back to `Plain`).
pub fn table_styled(headers: &[&str], rows: &[Vec<String>], styles: &[Style]) {
    if headers.is_empty() {
        return;
    }
    let w = width();
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (idx, cell) in row.iter().enumerate().take(cols) {
            widths[idx] = widths[idx].max(cell.chars().count());
        }
    }

    let fixed: usize = widths[..cols - 1].iter().sum::<usize>() + GAP * (cols - 1);
    let natural = fixed + widths[cols - 1];
    if natural > w {
        // Flex the last column into whatever room remains.
        widths[cols - 1] = w
            .saturating_sub(fixed)
            .max(16)
            .max(headers[cols - 1].chars().count());
    }

    let dim_all = vec![Style::Dim; cols];
    let headers_owned: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    print_wrapped_row(&headers_owned, &widths, &dim_all);
    let separators: Vec<String> = widths.iter().map(|cw| "-".repeat(*cw)).collect();
    print_wrapped_row(&separators, &widths, &dim_all);
    for row in rows {
        print_wrapped_row(row, &widths, styles);
    }
}

/// Print one row; the last column wraps onto continuation lines aligned under
/// its own start. Layout is computed on plain text; styles are applied per
/// printed fragment (padding stays outside the escape codes).
fn print_wrapped_row(cells: &[String], widths: &[usize], styles: &[Style]) {
    let style_of = |idx: usize| styles.get(idx).copied().unwrap_or(Style::Plain);
    let cols = widths.len();
    let mut line = String::new();
    let mut indent = 0usize;
    for (idx, cw) in widths.iter().enumerate().take(cols - 1) {
        let cell = cells.get(idx).map(String::as_str).unwrap_or("");
        line.push_str(&paint(style_of(idx), &format!("{cell:<cw$}")));
        line.push_str(&" ".repeat(GAP));
        indent += cw + GAP;
    }
    let last = cells.get(cols - 1).map(String::as_str).unwrap_or("");
    let wrapped = wrap(last, widths[cols - 1]);
    println!("{line}{}", paint(style_of(cols - 1), &wrapped[0]));
    for cont in &wrapped[1..] {
        println!("{}{}", " ".repeat(indent), paint(style_of(cols - 1), cont));
    }
}

/// `2026-07-06T18:25:25.713220Z` → `2026-07-06 18:25` (display only).
pub fn human_time(rfc3339: &str) -> String {
    if rfc3339.len() >= 16 && rfc3339.as_bytes()[10] == b'T' {
        let mut s: String = rfc3339.chars().take(16).collect();
        s.replace_range(10..11, " ");
        s
    } else {
        rfc3339.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_middle_keeps_head_and_tail() {
        let s = "/Users/alexis/workspace/perso/projects/software/dejavu/some/deep/dir";
        let t = truncate_middle(s, 30);
        assert_eq!(t.chars().count(), 30);
        assert!(t.ends_with("deep/dir"));
        assert!(t.contains('…'));
        assert_eq!(truncate_middle("short", 30), "short");
    }

    #[test]
    fn wrap_breaks_on_words_and_hard_breaks_long_tokens() {
        let lines = wrap("alpha beta gamma delta", 11);
        assert_eq!(lines, vec!["alpha beta", "gamma delta"]);
        let lines = wrap(&"x".repeat(25), 10);
        assert_eq!(lines, vec!["x".repeat(10), "x".repeat(10), "x".repeat(5)]);
    }

    #[test]
    fn human_time_shortens_rfc3339() {
        assert_eq!(
            human_time("2026-07-06T18:25:25.713220Z"),
            "2026-07-06 18:25"
        );
        assert_eq!(human_time("never"), "never");
    }
}
