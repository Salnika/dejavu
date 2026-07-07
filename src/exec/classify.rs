//! Command classification and passthrough policy (spec §11).
//!
//! Default is passthrough. A command is optimized only when the shim is known,
//! the family is supported, the args match a whitelist, it is not interactive,
//! Dejavu is not disabled, and config allows it. Anything ambiguous — in
//! particular any git subcommand not provably read-only — passes through.

use super::command_key::{build_key, command_original, drop_noise};
use super::interactive::{has_watch_flag, is_known_interactive};
use super::{ExecMode, Family, PassthroughReason};
use crate::config::Config;

use ExecMode::{Optimize, Passthrough};
use PassthroughReason as R;

pub struct Classified {
    pub mode: ExecMode,
    pub command_original: String,
}

/// Read-only git subcommands that may be optimized.
const GIT_READONLY: &[&str] = &["status", "diff", "log", "show"];
/// Git subcommands that must always pass through (spec §10.7).
const GIT_MUTATING: &[&str] = &[
    "add",
    "am",
    "apply",
    "branch",
    "checkout",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "fetch",
    "merge",
    "mv",
    "pull",
    "push",
    "rebase",
    "reset",
    "restore",
    "revert",
    "rm",
    "stash",
    "switch",
    "tag",
    "worktree",
];
/// JS package-manager scripts we optimize.
const JS_SCRIPTS: &[&str] = &["test", "lint", "typecheck", "build"];
/// Script-name prefixes we also optimize (`test:unit`, `lint:js`,
/// `build:prod`, `typecheck:strict`, …).
const JS_SCRIPT_PREFIXES: &[&str] = &["test:", "lint:", "build:", "typecheck:"];
/// `find` primaries that cause side effects.
const FIND_DANGEROUS: &[&str] = &[
    "-exec", "-execdir", "-delete", "-ok", "-okdir", "-fprint", "-fprintf", "-fls", "-fprint0",
];

pub fn classify(
    shim: &str,
    args: &[String],
    cfg: &Config,
    disabled: bool,
    repo_disabled: bool,
    _stdin_is_tty: bool,
) -> Classified {
    let command_original = command_original(shim, args);
    let wrap = |mode| Classified {
        mode,
        command_original: command_original.clone(),
    };

    if disabled {
        return wrap(Passthrough(R::Disabled));
    }
    if repo_disabled {
        return wrap(Passthrough(R::RepoDisabled));
    }
    if !cfg.intercept.is_enabled(shim) {
        return wrap(Passthrough(R::ConfigExcluded));
    }

    let mode = match shim {
        "npm" | "pnpm" | "yarn" | "bun" => classify_js(shim, args),
        "tsc" => classify_tsc(args),
        "eslint" => classify_eslint(args),
        "vitest" => classify_vitest(args),
        "jest" => classify_jest(args),
        "pytest" => classify_pytest(args),
        "cargo" => classify_cargo(args),
        "go" => classify_go(args),
        "rg" | "grep" => classify_search(shim, args),
        "find" => classify_find(args),
        "ls" | "tree" => classify_listing(shim, args),
        "git" => classify_git(args),
        "docker" => classify_docker(args),
        // User-elected `[intercept] extra` commands: generic validation
        // treatment (builtins above always take precedence).
        _ if cfg.intercept.is_extra(shim) => classify_extra(shim, args),
        _ => Passthrough(R::UnknownShim),
    };
    wrap(mode)
}

/// A user-added command from `[intercept] extra`. Reduced generically as a
/// validation-style command; the standard guards still apply (watch-mode
/// passthrough here, plus the min-token floor and agent gating downstream).
fn classify_extra(shim: &str, args: &[String]) -> ExecMode {
    if has_watch_flag(args) || is_known_interactive(shim, first_positional(args).map(|(_, s)| s)) {
        return Passthrough(R::Interactive);
    }
    Optimize {
        family: Family::Validation,
        command_key: build_key(&format!("validation:{shim}"), &drop_noise(args)),
    }
}

/// First token not starting with `-` (the subcommand), with its index.
fn first_positional(args: &[String]) -> Option<(usize, &str)> {
    args.iter()
        .enumerate()
        .find(|(_, a)| !a.starts_with('-'))
        .map(|(i, a)| (i, a.as_str()))
}

fn classify_js(shim: &str, args: &[String]) -> ExecMode {
    if has_watch_flag(args) {
        return Passthrough(R::Interactive);
    }
    let Some((idx, sub)) = first_positional(args) else {
        return Passthrough(R::UnsupportedSubcommand);
    };

    let (script, key_tokens): (&str, Vec<String>) = if sub == "run" {
        // `pnpm run <script>` — the script is the next positional.
        match args[idx + 1..].iter().find(|a| !a.starts_with('-')) {
            Some(s) => (s.as_str(), vec!["run".to_string(), s.clone()]),
            None => return Passthrough(R::UnsupportedSubcommand),
        }
    } else {
        (sub, vec![sub.to_string()])
    };

    if is_known_interactive(shim, Some(script)) {
        return Passthrough(R::Interactive);
    }
    if js_script_whitelisted(script) {
        Optimize {
            family: Family::Validation,
            command_key: build_key(&format!("validation:{shim}"), &key_tokens),
        }
    } else {
        Passthrough(R::UnsupportedSubcommand)
    }
}

/// Exact whitelist names, plus `test:*`/`lint:*`/`build:*`/`typecheck:*`
/// variants — unless a `:`-segment names a watch/serve/mutating flavor
/// (`test:watch`, `lint:fix`, `build:dev` stay passthrough; `test:fixtures`
/// is fine).
fn js_script_whitelisted(script: &str) -> bool {
    if JS_SCRIPTS.contains(&script) {
        return true;
    }
    JS_SCRIPT_PREFIXES.iter().any(|p| script.starts_with(p))
        && !script.split(':').any(|seg| {
            seg == "fix"
                || seg == "dev"
                || seg == "serve"
                || seg == "start"
                || seg.starts_with("watch")
        })
}

fn classify_tsc(args: &[String]) -> ExecMode {
    if has_watch_flag(args) || args.iter().any(|a| a == "-w" || a == "--watch") {
        return Passthrough(R::Interactive);
    }
    Optimize {
        family: Family::Validation,
        command_key: build_key("validation:tsc", &drop_noise(args)),
    }
}

fn classify_eslint(args: &[String]) -> ExecMode {
    if args.iter().any(|a| a == "--fix" || a == "--fix-dry-run") {
        return Passthrough(R::SideEffecting);
    }
    if has_watch_flag(args) {
        return Passthrough(R::Interactive);
    }
    Optimize {
        family: Family::Validation,
        command_key: build_key("validation:eslint", &drop_noise(args)),
    }
}

fn classify_pytest(args: &[String]) -> ExecMode {
    Optimize {
        family: Family::Validation,
        command_key: build_key("validation:pytest", &drop_noise(args)),
    }
}

/// `vitest` defaults to watch mode in a dev terminal, so only the explicit
/// single-run forms (`vitest run …`, `--run`) are optimized.
fn classify_vitest(args: &[String]) -> ExecMode {
    if has_watch_flag(args) {
        return Passthrough(R::Interactive);
    }
    // Snapshot updates rewrite files in the repo.
    if args.iter().any(|a| a == "-u" || a == "--update") {
        return Passthrough(R::SideEffecting);
    }
    let explicit_run =
        matches!(first_positional(args), Some((_, "run"))) || args.iter().any(|a| a == "--run");
    if explicit_run {
        Optimize {
            family: Family::Validation,
            command_key: build_key("validation:vitest", &drop_noise(args)),
        }
    } else {
        // Bare `vitest` / `vitest watch` / `vitest dev` live-rerun on changes.
        Passthrough(R::Interactive)
    }
}

/// `jest` runs once by default; watch and snapshot-update forms pass through.
fn classify_jest(args: &[String]) -> ExecMode {
    if has_watch_flag(args) {
        return Passthrough(R::Interactive);
    }
    if args.iter().any(|a| a == "-u" || a == "--updateSnapshot") {
        return Passthrough(R::SideEffecting);
    }
    Optimize {
        family: Family::Validation,
        command_key: build_key("validation:jest", &drop_noise(args)),
    }
}

fn classify_cargo(args: &[String]) -> ExecMode {
    // Skip a leading `+toolchain` selector.
    let rest: &[String] = match args.first() {
        Some(first) if first.starts_with('+') => &args[1..],
        _ => args,
    };
    match first_positional(rest) {
        Some((idx, "test")) => Optimize {
            family: Family::Validation,
            command_key: build_key("validation:cargo:test", &drop_noise(&rest[idx + 1..])),
        },
        Some((idx, "check")) => Optimize {
            family: Family::Validation,
            command_key: build_key("validation:cargo:check", &drop_noise(&rest[idx + 1..])),
        },
        Some((idx, "clippy")) => {
            // `cargo clippy --fix` rewrites source files.
            if rest.iter().any(|a| a == "--fix") {
                Passthrough(R::SideEffecting)
            } else {
                Optimize {
                    family: Family::Validation,
                    command_key: build_key(
                        "validation:cargo:clippy",
                        &drop_noise(&rest[idx + 1..]),
                    ),
                }
            }
        }
        _ => Passthrough(R::UnsupportedSubcommand),
    }
}

fn classify_go(args: &[String]) -> ExecMode {
    match first_positional(args) {
        Some((idx, "test")) => Optimize {
            family: Family::Validation,
            command_key: build_key("validation:go:test", &drop_noise(&args[idx + 1..])),
        },
        _ => Passthrough(R::UnsupportedSubcommand),
    }
}

fn classify_search(shim: &str, args: &[String]) -> ExecMode {
    // Unparseable machine formats degrade to generic later; still safe to capture.
    Optimize {
        family: Family::Search,
        command_key: build_key(&format!("search:{shim}"), &drop_noise(args)),
    }
}

fn classify_find(args: &[String]) -> ExecMode {
    if args.iter().any(|a| FIND_DANGEROUS.contains(&a.as_str())) {
        return Passthrough(R::SideEffecting);
    }
    Optimize {
        family: Family::Tree,
        command_key: build_key("tree:find", &drop_noise(args)),
    }
}

fn classify_listing(shim: &str, args: &[String]) -> ExecMode {
    Optimize {
        family: Family::Tree,
        command_key: build_key(&format!("tree:{shim}"), &drop_noise(args)),
    }
}

/// The git subcommand, skipping global options (`git -C p status`, `git -c k=v
/// commit`, etc.). Returns `(index, subcommand)`.
fn git_subcommand(args: &[String]) -> Option<(usize, &str)> {
    const TAKES_VALUE: &[&str] = &[
        "-C",
        "-c",
        "--git-dir",
        "--work-tree",
        "--namespace",
        "--exec-path",
        "--super-prefix",
    ];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            if TAKES_VALUE.contains(&a.as_str()) {
                i += 2; // separate-form value follows
            } else {
                i += 1; // flag, or `--opt=value` single token
            }
            continue;
        }
        return Some((i, a.as_str()));
    }
    None
}

fn classify_git(args: &[String]) -> ExecMode {
    match git_subcommand(args) {
        Some((idx, sub)) if GIT_READONLY.contains(&sub) => {
            // Machine-readable forms (`--porcelain`, `-z`, `@{upstream}` ranges,
            // `--no-ext-diff`) are run by shell prompts and IDE SCM, which PARSE
            // the output — reducing it would corrupt them. Pass those through and
            // only optimize the human-readable forms an agent actually reads.
            if is_machine_readable(&args[idx + 1..]) {
                Passthrough(R::MachineReadable)
            } else {
                Optimize {
                    family: Family::GitReadonly,
                    command_key: build_key(&format!("git:{sub}"), &drop_noise(&args[idx + 1..])),
                }
            }
        }
        Some((_, sub)) if GIT_MUTATING.contains(&sub) => Passthrough(R::MutatingGit),
        // Unknown or no subcommand → safe passthrough.
        _ => Passthrough(R::UnsupportedSubcommand),
    }
}

/// True when a git read-only invocation emits a stable, machine-parseable
/// format that a program (shell prompt, IDE SCM, git hook, or an agent's own
/// `$(...)`/pipe/xargs) consumes. Reducing those corrupts the parser, so they
/// pass through untouched. This is a block-list and so necessarily incomplete;
/// we err toward passthrough (a missed optimization is harmless, a corrupted
/// parse is not), and the min-token floor is the final backstop.
fn is_machine_readable(args: &[String]) -> bool {
    args.iter().any(|a| {
        matches!(
            a.as_str(),
            // stable columnar / scripting formats
            "-z" | "-s"
                | "--short"
                | "--name-only"
                | "--name-status"
                | "--numstat"
                | "--raw"
                | "--no-ext-diff"
        ) || a.starts_with("--porcelain")      // --porcelain, =v1, =v2
            || a.starts_with("--format")        // git log/show custom format
            || a.starts_with("--pretty=format") // --pretty=format:%H (machine)
            || a.contains("@{u") // @{u} / @{upstream} ahead-behind ranges
    })
}

/// First positional for docker, skipping global options that take a value.
fn docker_positional(args: &[String], from: usize) -> Option<(usize, &str)> {
    const TAKES_VALUE: &[&str] = &[
        "-H",
        "--host",
        "--context",
        "--config",
        "--log-level",
        "-l",
        "--tlscacert",
        "--tlscert",
        "--tlskey",
    ];
    let mut i = from;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with('-') {
            if TAKES_VALUE.contains(&a.as_str()) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        return Some((i, a.as_str()));
    }
    None
}

fn classify_docker(args: &[String]) -> ExecMode {
    match docker_positional(args, 0) {
        Some((idx, "logs")) => Optimize {
            family: Family::Logs,
            command_key: build_key("logs:docker:logs", &drop_noise(&args[idx + 1..])),
        },
        Some((idx, "compose")) => match docker_positional(args, idx + 1) {
            Some((jdx, "logs")) => Optimize {
                family: Family::Logs,
                command_key: build_key("logs:docker-compose:logs", &drop_noise(&args[jdx + 1..])),
            },
            _ => Passthrough(R::DangerousDocker),
        },
        _ => Passthrough(R::DangerousDocker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config::default()
    }

    fn a(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    fn mode(shim: &str, args: &[&str]) -> ExecMode {
        classify(shim, &a(args), &cfg(), false, false, false).mode
    }

    fn is_opt(m: &ExecMode) -> bool {
        matches!(m, Optimize { .. })
    }

    fn key(m: &ExecMode) -> String {
        match m {
            Optimize { command_key, .. } => command_key.clone(),
            _ => String::new(),
        }
    }

    #[test]
    fn js_validation_optimized() {
        assert!(is_opt(&mode("pnpm", &["test"])));
        assert!(is_opt(&mode("pnpm", &["run", "test"])));
        assert!(is_opt(&mode("npm", &["run", "lint"])));
        assert!(is_opt(&mode("yarn", &["build"])));
        assert!(is_opt(&mode("bun", &["test"])));
        assert_eq!(key(&mode("pnpm", &["test"])), "validation:pnpm:test");
        assert_eq!(
            key(&mode("pnpm", &["run", "typecheck"])),
            "validation:pnpm:run:typecheck"
        );
    }

    #[test]
    fn js_side_effect_commands_passthrough() {
        assert!(!is_opt(&mode("pnpm", &["install"])));
        assert!(!is_opt(&mode("npm", &["publish"])));
        assert!(!is_opt(&mode("yarn", &["add", "react"])));
        assert!(!is_opt(&mode("bun", &["add", "zod"])));
    }

    #[test]
    fn watch_mode_passthrough() {
        assert!(matches!(
            mode("pnpm", &["test", "--watch"]),
            Passthrough(R::Interactive)
        ));
        assert!(matches!(
            mode("tsc", &["--watch"]),
            Passthrough(R::Interactive)
        ));
    }

    #[test]
    fn git_readonly_optimized_mutating_passthrough() {
        assert!(is_opt(&mode("git", &["diff"])));
        assert!(is_opt(&mode("git", &["status"])));
        assert!(is_opt(&mode("git", &["log"])));
        assert!(is_opt(&mode("git", &["show"])));
        assert_eq!(key(&mode("git", &["diff"])), "git:diff:default");

        for sub in GIT_MUTATING {
            assert!(
                matches!(mode("git", &[sub]), Passthrough(R::MutatingGit)),
                "git {sub} must pass through"
            );
        }
    }

    #[test]
    fn extra_commands_classify_as_generic_validation() {
        let mut cfg = Config::default();
        cfg.intercept.extra = vec!["mytool".to_string()];

        // Optimized with a validation key.
        let c = classify(
            "mytool",
            &["run".to_string(), "--color=always".to_string()],
            &cfg,
            false,
            false,
            false,
        );
        match &c.mode {
            Optimize {
                family,
                command_key,
            } => {
                assert_eq!(*family, Family::Validation);
                assert_eq!(command_key, "validation:mytool:run");
            }
            other => panic!("expected Optimize, got {other:?}"),
        }

        // Watch mode passes through.
        let c = classify(
            "mytool",
            &["--watch".to_string()],
            &cfg,
            false,
            false,
            false,
        );
        assert!(matches!(c.mode, Passthrough(R::Interactive)));

        // Not in extra -> excluded by the config gate (never optimized).
        let c = classify("randomtool", &[], &cfg, false, false, false);
        assert!(matches!(
            c.mode,
            Passthrough(R::ConfigExcluded | R::UnknownShim)
        ));

        // A builtin listed in extra keeps its specialized policy.
        cfg.intercept.extra.push("git".to_string());
        let c = classify("git", &["commit".to_string()], &cfg, false, false, false);
        assert!(matches!(c.mode, Passthrough(R::MutatingGit)));
    }

    #[test]
    fn git_machine_forms_passthrough_human_forms_optimize() {
        // Prompt / IDE SCM / scripting forms — parsed by programs, must pass through.
        for args in [
            &["status", "--porcelain"][..],
            &["status", "--porcelain=v2", "-z"][..],
            &["status", "-s"][..],
            &["status", "--short"][..],
            &["diff", "--no-ext-diff", "--ignore-submodules"][..],
            &["diff", "--name-only"][..],
            &["diff", "--numstat"][..],
            &["diff", "--name-status", "--diff-filter=ACM"][..],
            &["log", "--oneline", "..@{upstream}"][..],
            &["log", "-1", "--format=%H"][..],
            &["log", "--pretty=format:%h %s"][..],
        ] {
            assert!(
                matches!(mode("git", args), Passthrough(R::MachineReadable)),
                "git {args:?} must pass through (machine-readable)"
            );
        }
        // Human/agent forms — still optimized.
        assert!(is_opt(&mode("git", &["status"])));
        assert!(is_opt(&mode("git", &["diff"])));
        assert!(is_opt(&mode("git", &["diff", "HEAD~1"])));
        assert!(is_opt(&mode("git", &["diff", "--stat"])));
        assert!(is_opt(&mode("git", &["log", "--oneline", "-5"])));
        assert!(is_opt(&mode("git", &["log", "--graph"])));
        assert!(is_opt(&mode("git", &["-C", "/tmp/r", "status"])));
    }

    #[test]
    fn git_global_options_before_subcommand() {
        assert!(is_opt(&mode("git", &["-C", "/tmp/repo", "diff"])));
        assert!(matches!(
            mode("git", &["-C", "/tmp/repo", "commit"]),
            Passthrough(R::MutatingGit)
        ));
        assert!(matches!(
            mode("git", &["-c", "user.name=x", "push"]),
            Passthrough(R::MutatingGit)
        ));
    }

    #[test]
    fn docker_logs_optimized_run_passthrough() {
        assert!(is_opt(&mode("docker", &["logs", "api"])));
        assert!(is_opt(&mode("docker", &["compose", "logs", "api"])));
        assert!(matches!(
            mode("docker", &["run", "img"]),
            Passthrough(R::DangerousDocker)
        ));
        assert!(matches!(
            mode("docker", &["compose", "up"]),
            Passthrough(R::DangerousDocker)
        ));
    }

    #[test]
    fn find_dangerous_passthrough() {
        assert!(is_opt(&mode("find", &[".", "-name", "*.ts"])));
        assert!(matches!(
            mode("find", &[".", "-delete"]),
            Passthrough(R::SideEffecting)
        ));
        assert!(matches!(
            mode("find", &[".", "-exec", "rm", "{}", ";"]),
            Passthrough(R::SideEffecting)
        ));
    }

    #[test]
    fn eslint_fix_passthrough() {
        assert!(is_opt(&mode("eslint", &["."])));
        assert!(matches!(
            mode("eslint", &[".", "--fix"]),
            Passthrough(R::SideEffecting)
        ));
    }

    #[test]
    fn cargo_go_validation_subcommands() {
        assert!(is_opt(&mode("cargo", &["test"])));
        assert!(is_opt(&mode("cargo", &["+nightly", "test"])));
        assert!(is_opt(&mode("cargo", &["check"])));
        assert!(is_opt(&mode("cargo", &["check", "--all-targets"])));
        assert!(is_opt(&mode("cargo", &["clippy"])));
        assert!(is_opt(&mode("cargo", &["clippy", "--", "-D", "warnings"])));
        assert_eq!(
            key(&mode("cargo", &["check"])),
            "validation:cargo:check:default"
        );
        assert!(!is_opt(&mode("cargo", &["build"])));
        assert!(!is_opt(&mode("cargo", &["publish"])));
        // `clippy --fix` rewrites source files.
        assert!(matches!(
            mode("cargo", &["clippy", "--fix"]),
            Passthrough(R::SideEffecting)
        ));
        assert!(is_opt(&mode("go", &["test", "./..."])));
        assert!(!is_opt(&mode("go", &["build"])));
    }

    #[test]
    fn vitest_only_explicit_run_optimized() {
        assert!(is_opt(&mode("vitest", &["run"])));
        assert!(is_opt(&mode("vitest", &["run", "src/session"])));
        assert!(is_opt(&mode("vitest", &["related", "--run", "src/a.ts"])));
        assert_eq!(key(&mode("vitest", &["run"])), "validation:vitest:run");
        // Bare vitest defaults to watch mode in a dev terminal.
        assert!(matches!(mode("vitest", &[]), Passthrough(R::Interactive)));
        assert!(matches!(
            mode("vitest", &["watch"]),
            Passthrough(R::Interactive)
        ));
        assert!(matches!(
            mode("vitest", &["run", "--watch"]),
            Passthrough(R::Interactive)
        ));
        // Snapshot updates rewrite files.
        assert!(matches!(
            mode("vitest", &["run", "-u"]),
            Passthrough(R::SideEffecting)
        ));
    }

    #[test]
    fn jest_optimized_except_watch_and_snapshot_updates() {
        assert!(is_opt(&mode("jest", &[])));
        assert!(is_opt(&mode("jest", &["src/session"])));
        assert_eq!(key(&mode("jest", &[])), "validation:jest:default");
        assert!(matches!(
            mode("jest", &["--watch"]),
            Passthrough(R::Interactive)
        ));
        assert!(matches!(
            mode("jest", &["--watchAll"]),
            Passthrough(R::Interactive)
        ));
        assert!(matches!(
            mode("jest", &["-u"]),
            Passthrough(R::SideEffecting)
        ));
        assert!(matches!(
            mode("jest", &["--updateSnapshot"]),
            Passthrough(R::SideEffecting)
        ));
    }

    #[test]
    fn js_script_prefixes_optimized_dangerous_variants_passthrough() {
        // `test:*` / `lint:*` / `build:*` / `typecheck:*` variants optimize.
        assert!(is_opt(&mode("pnpm", &["test:unit"])));
        assert!(is_opt(&mode("pnpm", &["run", "test:e2e"])));
        assert!(is_opt(&mode("npm", &["run", "lint:js"])));
        assert!(is_opt(&mode("yarn", &["build:prod"])));
        assert!(is_opt(&mode("pnpm", &["run", "typecheck:strict"])));
        // Segment equality spares look-alikes…
        assert!(is_opt(&mode("pnpm", &["run", "test:fixtures"])));
        // …but watch / fix / dev / serve / start variants stay passthrough.
        assert!(!is_opt(&mode("pnpm", &["run", "test:watch"])));
        assert!(!is_opt(&mode("pnpm", &["run", "lint:fix"])));
        assert!(!is_opt(&mode("pnpm", &["run", "build:dev"])));
        assert!(!is_opt(&mode("npm", &["run", "build:serve"])));
        assert!(!is_opt(&mode("pnpm", &["run", "test:watch-unit"])));
        // Unrelated script names are still unsupported.
        assert!(!is_opt(&mode("pnpm", &["run", "deploy:prod"])));
    }

    #[test]
    fn noise_flags_excluded_from_key() {
        assert_eq!(
            key(&mode("rg", &["--color=always", "createSession", "src"])),
            "search:rg:createSession:src"
        );
    }

    #[test]
    fn disabled_and_config_excluded() {
        let disabled = classify("pnpm", &a(&["test"]), &cfg(), true, false, false).mode;
        assert!(matches!(disabled, Passthrough(R::Disabled)));

        let mut c = cfg();
        c.intercept.git = false;
        let excluded = classify("git", &a(&["diff"]), &c, false, false, false).mode;
        assert!(matches!(excluded, Passthrough(R::ConfigExcluded)));
    }
}
