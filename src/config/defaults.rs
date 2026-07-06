//! Default effective config (spec §18).

use super::{Config, InterceptConfig};

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            store_raw_outputs: true,
            redact_secrets: true,
            max_raw_output_bytes: 5_242_880,
            min_raw_tokens_to_reduce: 800,
            max_emitted_lines_first_seen: 160,
            max_emitted_lines_large_delta: 160,
            max_emitted_lines_small_delta: 120,
            small_delta_max_changed_lines: 80,
            small_delta_max_changed_ratio: 0.20,
            estimate_tokens_method: "chars_div_4".to_string(),
            retention_days: 14,
            intercept: InterceptConfig::default(),
        }
    }
}

impl Default for InterceptConfig {
    fn default() -> Self {
        // Every shim is intercepted by default.
        InterceptConfig {
            npm: true,
            pnpm: true,
            yarn: true,
            bun: true,
            tsc: true,
            eslint: true,
            pytest: true,
            cargo: true,
            go: true,
            git: true,
            rg: true,
            grep: true,
            find: true,
            ls: true,
            tree: true,
            docker: true,
        }
    }
}
