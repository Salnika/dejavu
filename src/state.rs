//! Per-repo enable/disable state, stored in the cache (spec §17.9) — never in
//! the repo. Precedence: `DEJAVU_DISABLED` env > this state > `config.enabled`.

use crate::paths::CacheLayout;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoState {
    pub disabled: bool,
}

pub fn load(layout: &CacheLayout) -> RepoState {
    std::fs::read_to_string(layout.state_file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(layout: &CacheLayout, state: &RepoState) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(state).expect("RepoState always serializes");
    std::fs::write(layout.state_file(), json)
}

pub fn is_repo_disabled(layout: &CacheLayout) -> bool {
    load(layout).disabled
}
