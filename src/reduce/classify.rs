//! The reduction state machine (spec §12). This is the single definition of
//! `Classification`, consumed by the reducer and stored as a string.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    FirstSeen,
    Unchanged,
    SmallDelta,
    LargeDelta,
}

impl Classification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Classification::FirstSeen => "first_seen",
            Classification::Unchanged => "unchanged",
            Classification::SmallDelta => "small_delta",
            Classification::LargeDelta => "large_delta",
        }
    }
}
