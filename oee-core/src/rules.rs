//! Core constants and counting rules.

use serde::{Deserialize, Serialize};

/// FR-001, FR-002: the default cycle time, in minutes. A day's own cycle time
/// is stored on the day (D-61); this is the default for the setting and for
/// day files saved before the setting existed.
pub const BLOCK_MINUTES: u32 = 20;

/// The shortest cycle time the setting accepts.
pub const MIN_CYCLE_MINUTES: u32 = 5;

/// The longest cycle time the setting accepts.
pub const MAX_CYCLE_MINUTES: u32 = 100;

pub(crate) fn default_cycle() -> u32 {
    BLOCK_MINUTES
}

/// Keep a cycle time read from a file inside the accepted range, so a hand-edited
/// 0 (divide by zero) or a huge value (overflow) can't crash the app.
pub fn clamp_cycle(cycle: u32) -> u32 {
    cycle.clamp(MIN_CYCLE_MINUTES, MAX_CYCLE_MINUTES)
}

/// `deserialize_with` for `cycle_minutes` fields: reads the number, then clamps it.
pub(crate) fn deserialize_cycle<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    u32::deserialize(d).map(clamp_cycle)
}

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
    lost_blocks_in(duration, BLOCK_MINUTES)
}

/// [`lost_blocks`] for a day with its own cycle time. The 5-minute leeway is a
/// quarter of the cycle, so it stays 5 at the default 20.
pub fn lost_blocks_in(duration: u32, cycle: u32) -> u32 {
    let full = duration / cycle;
    let remainder = duration % cycle;
    full + u32::from(remainder > cycle / 4)
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
    /// D-61: minutes in one block. A day copies this when it starts.
    #[serde(default = "default_cycle", deserialize_with = "deserialize_cycle")]
    pub cycle_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_start: hm(9, 0),
            default_end: hm(17, 40),
            base_target: 20,
            cycle_minutes: BLOCK_MINUTES,
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
        let gained = (early_start + late_finish) / self.cycle_minutes;
        let missed = (late_start + early_finish) / self.cycle_minutes;
        (self.base_target + gained).saturating_sub(missed)
    }
}
