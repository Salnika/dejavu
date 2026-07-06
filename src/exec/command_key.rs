//! `command_key` helpers: strip cosmetic noise, keep significant args.

/// Purely cosmetic flags that don't change *what* ran (spec §11.2).
pub const NOISE_FLAGS: &[&str] = &[
    "--color",
    "--colour",
    "--no-color",
    "--no-colour",
    "--progress",
    "--no-progress",
];

pub fn is_noise(arg: &str) -> bool {
    NOISE_FLAGS
        .iter()
        .any(|n| arg == *n || arg.starts_with(&format!("{n}=")))
}

/// Drop noise flags, keeping the rest in order.
pub fn drop_noise(args: &[String]) -> Vec<String> {
    args.iter().filter(|a| !is_noise(a)).cloned().collect()
}

/// Human-facing command string, e.g. `pnpm run test`.
pub fn command_original(shim: &str, args: &[String]) -> String {
    if args.is_empty() {
        shim.to_string()
    } else {
        format!("{shim} {}", args.join(" "))
    }
}

/// Build a colon-joined key from a family prefix and significant tokens,
/// falling back to `default` when there are none.
pub fn build_key(prefix: &str, sig: &[String]) -> String {
    if sig.is_empty() {
        format!("{prefix}:default")
    } else {
        format!("{prefix}:{}", sig.join(":"))
    }
}
