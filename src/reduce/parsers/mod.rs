//! Specialized reducers (spec §16). Each produces a compact, family-specific
//! body; anything unparseable degrades to the generic reducer (returns `None`).

pub mod dockerlogs;
pub mod eslint;
pub mod findings;
pub mod gitdiff;
pub mod gitstatus;
pub mod jest;
pub mod listing;
pub mod pytest;
pub mod search;
pub mod tsc;

use crate::reduce::Classification;

/// The compact output a specialized reducer produces. The envelope wraps this
/// with the command/exit-code header, suppressed-tokens footer, and ids.
pub struct SpecialOutput {
    pub status: String,
    pub body: String,
    pub full_label: &'static str,
    pub summary: String,
}

/// Dispatch to a specialized reducer. `None` → fall back to the generic reducer.
#[allow(clippy::too_many_arguments)]
pub fn dispatch(
    family: &str,
    shim: &str,
    command_key: &str,
    normalized: &str,
    prior: Option<&str>,
    class: Classification,
    exit_code: i32,
    prev_short: Option<&str>,
    max_lines: usize,
) -> Option<SpecialOutput> {
    match family {
        "validation" => validation(
            shim, normalized, prior, class, exit_code, prev_short, max_lines,
        ),
        "search" => search::reduce(normalized, prior, class, prev_short, max_lines),
        "tree" => listing::reduce(normalized, prior, class, prev_short, max_lines),
        "git_readonly" => {
            if command_key.starts_with("git:diff") {
                gitdiff::reduce(normalized, prior, class, prev_short, max_lines)
            } else if command_key.starts_with("git:status") {
                gitstatus::reduce(normalized, prior, class, prev_short, max_lines)
            } else {
                None // git log / show → generic
            }
        }
        "logs" => dockerlogs::reduce(normalized, prior, class, prev_short, max_lines),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum VKind {
    Tsc,
    Eslint,
    Pytest,
    Jest,
}

fn pick_validation(shim: &str, normalized: &str) -> Option<VKind> {
    if shim == "tsc" || tsc::looks_like(normalized) {
        Some(VKind::Tsc)
    } else if shim == "eslint" || eslint::looks_like(normalized) {
        Some(VKind::Eslint)
    } else if pytest::looks_like(normalized) {
        Some(VKind::Pytest)
    } else if jest::looks_like(normalized) {
        Some(VKind::Jest)
    } else {
        None
    }
}

fn parse_validation(kind: VKind, text: &str) -> Vec<findings::Finding> {
    match kind {
        VKind::Tsc => tsc::parse(text),
        VKind::Eslint => eslint::parse(text),
        VKind::Pytest => pytest::parse(text),
        VKind::Jest => jest::parse(text),
    }
}

fn labels_for(kind: VKind) -> findings::Labels {
    match kind {
        VKind::Tsc => tsc::labels(),
        VKind::Eslint => eslint::labels(),
        VKind::Pytest => pytest::labels(),
        VKind::Jest => jest::labels(),
    }
}

fn validation(
    shim: &str,
    normalized: &str,
    prior: Option<&str>,
    class: Classification,
    exit_code: i32,
    prev_short: Option<&str>,
    max_lines: usize,
) -> Option<SpecialOutput> {
    let kind = pick_validation(shim, normalized)?;
    let current = parse_validation(kind, normalized);
    // No parseable findings (e.g. a passing run, or a build log) → generic.
    if current.is_empty() {
        return None;
    }
    let prior_findings = prior.map(|p| parse_validation(kind, p));
    Some(findings::compose(
        &labels_for(kind),
        &current,
        prior_findings.as_deref(),
        class,
        exit_code,
        prev_short,
        max_lines,
    ))
}
