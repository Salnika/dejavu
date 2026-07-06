//! Effective configuration (spec §18): defaults ← global `config.toml` ←
//! project `.dejavu.toml`. Each layer supplies only the keys it overrides.

mod defaults;

use crate::error::ConfigError;
use crate::paths::{config_file_path, CacheLayout};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Canonical list of interceptable command names (spec §10). This is the single
/// source of truth; `exec::shim` generates a shim for each enabled entry.
pub const SHIM_NAMES: &[&str] = &[
    "npm", "pnpm", "yarn", "bun", "git", "rg", "grep", "find", "ls", "tree", "tsc", "eslint",
    "pytest", "cargo", "go", "docker",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub store_raw_outputs: bool,
    pub redact_secrets: bool,
    pub max_raw_output_bytes: u64,
    pub min_raw_tokens_to_reduce: u64,
    pub max_emitted_lines_first_seen: usize,
    pub max_emitted_lines_large_delta: usize,
    pub max_emitted_lines_small_delta: usize,
    pub small_delta_max_changed_lines: usize,
    pub small_delta_max_changed_ratio: f64,
    pub estimate_tokens_method: String,
    pub retention_days: u32,
    pub intercept: InterceptConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct InterceptConfig {
    pub npm: bool,
    pub pnpm: bool,
    pub yarn: bool,
    pub bun: bool,
    pub tsc: bool,
    pub eslint: bool,
    pub pytest: bool,
    pub cargo: bool,
    pub go: bool,
    pub git: bool,
    pub rg: bool,
    pub grep: bool,
    pub find: bool,
    pub ls: bool,
    pub tree: bool,
    pub docker: bool,
}

impl InterceptConfig {
    /// Whether a given shim name is enabled for interception.
    pub fn is_enabled(&self, shim: &str) -> bool {
        match shim {
            "npm" => self.npm,
            "pnpm" => self.pnpm,
            "yarn" => self.yarn,
            "bun" => self.bun,
            "tsc" => self.tsc,
            "eslint" => self.eslint,
            "pytest" => self.pytest,
            "cargo" => self.cargo,
            "go" => self.go,
            "git" => self.git,
            "rg" => self.rg,
            "grep" => self.grep,
            "find" => self.find,
            "ls" => self.ls,
            "tree" => self.tree,
            "docker" => self.docker,
            _ => false,
        }
    }

    /// The enabled shim names, in a stable order.
    pub fn enabled_shims(&self) -> Vec<&'static str> {
        SHIM_NAMES
            .iter()
            .copied()
            .filter(|name| self.is_enabled(name))
            .collect()
    }
}

/// Deep-merge `over` into `base` (tables merge key-by-key; scalars replace).
fn merge_toml(base: &mut toml::Value, over: toml::Value) {
    match over {
        toml::Value::Table(over_table) => {
            if let toml::Value::Table(base_table) = base {
                for (key, value) in over_table {
                    match base_table.get_mut(&key) {
                        Some(existing) => merge_toml(existing, value),
                        None => {
                            base_table.insert(key, value);
                        }
                    }
                }
            } else {
                *base = toml::Value::Table(over_table);
            }
        }
        other => *base = other,
    }
}

impl Config {
    /// Load and merge the effective config for a repo.
    pub fn load(repo_root: &Path) -> Result<Config, ConfigError> {
        let mut merged =
            toml::Value::try_from(Config::default()).expect("default config always serializes");

        if let Ok(path) = config_file_path() {
            if path.exists() {
                let text = std::fs::read_to_string(&path)?;
                let value: toml::Value =
                    toml::from_str(&text).map_err(|source| ConfigError::Toml {
                        path: path.clone(),
                        source,
                    })?;
                merge_toml(&mut merged, value);
            }
        }

        let project = repo_root.join(".dejavu.toml");
        if project.exists() {
            let text = std::fs::read_to_string(&project)?;
            let value: toml::Value = toml::from_str(&text).map_err(|source| ConfigError::Toml {
                path: project.clone(),
                source,
            })?;
            merge_toml(&mut merged, value);
        }

        merged.try_into().map_err(|source| ConfigError::Toml {
            path: repo_root.to_path_buf(),
            source,
        })
    }

    /// Serialize the effective config to `config.effective.json` for `doctor`.
    pub fn write_effective(&self, layout: &CacheLayout) -> Result<(), ConfigError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(layout.effective_config(), json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec_section_18() {
        let c = Config::default();
        assert!(c.enabled);
        assert!(c.store_raw_outputs);
        assert!(c.redact_secrets);
        assert_eq!(c.max_raw_output_bytes, 5_242_880);
        assert_eq!(c.min_raw_tokens_to_reduce, 800);
        assert_eq!(c.small_delta_max_changed_lines, 80);
        assert!((c.small_delta_max_changed_ratio - 0.20).abs() < 1e-9);
        assert_eq!(c.retention_days, 14);
        assert!(c.intercept.is_enabled("git"));
        assert!(c.intercept.is_enabled("docker"));
        assert!(!c.intercept.is_enabled("unknown-tool"));
    }

    #[test]
    fn merge_overrides_only_given_keys() {
        let mut base = toml::Value::try_from(Config::default()).unwrap();
        let over: toml::Value =
            toml::from_str("min_raw_tokens_to_reduce = 100\n[intercept]\ndocker = false\n")
                .unwrap();
        merge_toml(&mut base, over);
        let c: Config = base.try_into().unwrap();

        assert_eq!(c.min_raw_tokens_to_reduce, 100); // overridden
        assert!(!c.intercept.docker); // overridden
        assert!(c.intercept.git); // untouched by the overlay
        assert!(c.enabled); // untouched
        assert_eq!(c.retention_days, 14); // untouched
    }

    #[test]
    fn enabled_shims_reflects_intercept() {
        let mut c = Config::default();
        c.intercept.docker = false;
        c.intercept.go = false;
        let shims = c.intercept.enabled_shims();
        assert!(shims.contains(&"git"));
        assert!(!shims.contains(&"docker"));
        assert!(!shims.contains(&"go"));
        assert_eq!(shims.len(), SHIM_NAMES.len() - 2);
    }
}
