//! TypeScript compiler diagnostics (spec §16.3).

use super::findings::{Finding, Labels};
use regex::Regex;
use std::sync::LazyLock;

// Both `file(line,col): error TSxxxx: msg` and `file:line:col - error TSxxxx: msg`.
// The `[-:]?` after the location absorbs the `):` of the first form and the ` - `
// of the second.
static TS_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(.+?)[(:](\d+)[,:](\d+)\)?\s*[-:]?\s*error (TS\d+): (.*)$").unwrap()
});

pub fn looks_like(normalized: &str) -> bool {
    TS_ERROR.is_match(normalized)
}

pub fn parse(normalized: &str) -> Vec<Finding> {
    TS_ERROR
        .captures_iter(normalized)
        .map(|c| Finding {
            file: c[1].trim().to_string(),
            loc: format!("{}:{}", &c[2], &c[3]),
            code: c[4].to_string(),
            detail: Some(c[5].trim().to_string()),
        })
        .collect()
}

pub fn labels() -> Labels {
    Labels {
        noun: "typecheck",
        fail_status: "typecheck failed.",
        ok_status: "typecheck passed.",
        item_plural: "TypeScript errors",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_error_forms() {
        let paren = "src/auth/session.ts(84,12): error TS2322: Type 'string' is not assignable.";
        let colon = "src/api/user.ts:42:3 - error TS7006: Parameter 'req' implicitly any.";
        let findings = parse(&format!("{paren}\n{colon}"));
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].file, "src/auth/session.ts");
        assert_eq!(findings[0].loc, "84:12");
        assert_eq!(findings[0].code, "TS2322");
        assert_eq!(findings[1].file, "src/api/user.ts");
        assert_eq!(findings[1].code, "TS7006");
        assert!(looks_like(paren));
    }
}
