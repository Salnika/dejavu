//! Interactive / watch / TTY detection (spec §21.3).

use std::io::IsTerminal;

pub fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// Watch-mode flags that keep a process running (→ passthrough). Note `-w` is
/// deliberately excluded: for `grep`/`find` it means "word"/"writable", not watch.
pub fn has_watch_flag(args: &[String]) -> bool {
    args.iter()
        .any(|a| matches!(a.as_str(), "--watch" | "--watchAll" | "--watch=true"))
}

/// `(shim, subcommand)` pairs that inherently prompt / hold the terminal.
pub fn is_known_interactive(shim: &str, subcommand: Option<&str>) -> bool {
    matches!(
        (shim, subcommand),
        ("npm", Some("init"))
            | ("npm", Some("login"))
            | ("npm", Some("adduser"))
            | ("yarn", Some("login"))
            | ("docker", Some("exec"))
            | ("docker", Some("attach"))
    )
}
