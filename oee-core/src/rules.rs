//! Core constants and counting rules.

use serde::{Deserialize, Serialize};

/// FR-001, FR-002: every block is a fixed 20 minutes.
pub const BLOCK_MINUTES: u32 = 20;

/// FR-014: downtime of this many minutes or less is ignored and counts as Work.
pub const LEEWAY_MINUTES: u32 = 5;

/// FR-021: each Rework mark counts as half a loss in Quality.
pub const REWORK_LOSS: f64 = 0.5;

/// Minutes since midnight for `h:m`.
pub const fn hm(h: u32, m: u32) -> u32 {
    h * 60 + m
}

/// Number of lost blocks for a downtime stop of `duration` minutes.
///
/// FR-013, FR-014, FR-016, FR-017: every full 20 minutes is one block, and a
/// remainder over 5 minutes adds one more. Stops of 5 minutes or less are 0.
///
/// | Stop | Blocks |
/// |------|--------|
/// | ≤ 5  | 0      |
/// | 10   | 1      |
/// | 30   | 2      |
/// | 45   | 2      |
pub fn lost_blocks(duration: u32) -> u32 {
    let full = duration / BLOCK_MINUTES;
    let remainder = duration % BLOCK_MINUTES;
    full + u32::from(remainder > LEEWAY_MINUTES)
}

/// User-level defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// FR-004: default start, 09:00.
    pub default_start: u32,
    /// FR-004: default finish, 17:40 (26 whole blocks).
    pub default_end: u32,
    /// FR-046: default target Work points per day.
    pub base_target: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_start: hm(9, 0),
            default_end: hm(17, 40),
            base_target: 20,
        }
    }
}

impl Settings {
    /// Daily target Work points for a day running `start..end`.
    ///
    /// FR-046, FR-047, FR-047a: base target, plus 1 per extra 20 minutes from an
    /// early start or a late finish, minus 1 per 20 minutes missed through a late
    /// start or an early finish. Part-blocks don't count either way.
    pub fn target_points(&self, start: u32, end: u32) -> u32 {
        let early_start = self.default_start.saturating_sub(start);
        let late_finish = end.saturating_sub(self.default_end);
        let late_start = start.saturating_sub(self.default_start);
        let early_finish = self.default_end.saturating_sub(end);
        let gained = (early_start + late_finish) / BLOCK_MINUTES;
        let missed = (late_start + early_finish) / BLOCK_MINUTES;
        (self.base_target + gained).saturating_sub(missed)
    }
}
