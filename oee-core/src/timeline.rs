//! A day's 20-minute blocks.
//!
//! A [`DayRecord`] stores only the facts: the day window, the planned windows,
//! the downtime stops, and the user's edits (categories, rework, notes). The
//! blocks are rebuilt from those facts every time, so changing the end time or
//! recategorising never leaves stale state behind.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fmt;

use crate::calendar::PlannedWindow;
use crate::category::{LossMinutes, LostCategory, PlannedKind};
use crate::metrics::DayInputs;
use crate::rules::{lost_blocks, Settings, BLOCK_MINUTES};

/// A downtime stop, from shortcut-on to shortcut-off (FR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DowntimeEvent {
    pub start: u32,
    pub end: u32,
}

/// What the user has changed on a block, keyed by the block's start minute.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct BlockEdit {
    category: Option<LostCategory>,
    rework: bool,
    note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// FR-008, FR-019: Work, optionally marked as Rework.
    Work { rework: bool },
    /// FR-012: lost time. `None` until categorised.
    Lost { category: Option<LostCategory> },
    /// FR-033: auto-tagged Planned Downtime.
    Planned(PlannedKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub start: u32,
    pub end: u32,
    pub kind: BlockKind,
    /// FR-025.
    pub note: Option<String>,
}

impl Block {
    /// The category used in the maths. FR-027: uncategorised counts as System Down.
    pub fn effective_category(&self) -> Option<LostCategory> {
        match self.kind {
            BlockKind::Lost { category } => Some(category.unwrap_or(LostCategory::SystemDown)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// No block starts at that minute.
    NoSuchBlock,
    /// Categories only apply to lost-time blocks.
    NotLostTime,
    /// Rework only applies to Work blocks (FR-019).
    NotWork,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoSuchBlock => "no block starts at that time",
            Self::NotLostTime => "only lost-time blocks can be categorised",
            Self::NotWork => "only Work blocks can be marked as Rework",
        })
    }
}

impl std::error::Error for EditError {}

/// Everything recorded for one working day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayRecord {
    pub start: u32,
    pub end: u32,
    pub planned: Vec<PlannedWindow>,
    pub downtime: Vec<DowntimeEvent>,
    /// FR-027: set once the user has finished the day.
    #[serde(default)]
    pub finalised: bool,
    /// Start of a stop that is still running, so a restart can pick it up.
    /// Kept in step with the mode by the tracker.
    #[serde(default)]
    pub down_since: Option<u32>,
    edits: BTreeMap<u32, BlockEdit>,
}

impl DayRecord {
    /// Planned windows are snapped outwards to the day's 20-minute grid, so a
    /// 09:30–10:00 meeting on a 09:00 start becomes 09:20–10:00 (40 min).
    pub fn new(start: u32, end: u32, planned: Vec<PlannedWindow>) -> Self {
        assert!(end > start, "day must end after it starts");
        let mut planned: Vec<_> = planned.into_iter().map(|w| snap(w, start)).collect();
        planned.sort_by_key(|w| w.start);
        Self {
            start,
            end,
            planned,
            downtime: Vec::new(),
            finalised: false,
            down_since: None,
            edits: BTreeMap::new(),
        }
    }

    pub fn add_downtime(&mut self, start: u32, end: u32) {
        if end > start {
            self.downtime.push(DowntimeEvent { start, end });
        }
    }

    /// Number of whole 20-minute slots in the day.
    pub fn slot_count(&self) -> u32 {
        (self.end - self.start) / BLOCK_MINUTES
    }

    /// FR-032: planned minutes inside the day, overlaps merged.
    pub fn planned_minutes(&self) -> u32 {
        self.planned_minutes_until(self.end)
    }

    /// Planned minutes that have happened by `until`.
    fn planned_minutes_until(&self, until: u32) -> u32 {
        let end = self.end.min(until);
        let mut total = 0;
        let mut covered_to = self.start;
        for w in &self.planned {
            let s = w.start.max(covered_to).max(self.start);
            let e = w.end.min(end);
            if e > s {
                total += e - s;
                covered_to = e;
            }
        }
        total
    }

    /// The part of a stop that counts: inside the day, and never inside Planned
    /// Downtime. FR-036: a planned window starting mid-stop ends the stop there.
    fn effective(&self, ev: DowntimeEvent) -> Option<(u32, u32)> {
        let mut s = ev.start.max(self.start);
        let mut e = ev.end.min(self.end);
        for w in &self.planned {
            if w.contains(s) {
                s = w.end;
            }
            if w.start > s && w.start < e {
                e = w.start;
            }
        }
        (e > s).then_some((s, e))
    }

    /// All blocks for the whole day.
    pub fn blocks(&self) -> Vec<Block> {
        self.blocks_until(self.end)
    }

    /// Blocks that have finished by `until`.
    pub fn blocks_until(&self, until: u32) -> Vec<Block> {
        let n = self.slot_count() as usize;
        let slot = |i: usize| {
            let s = self.start + i as u32 * BLOCK_MINUTES;
            (s, s + BLOCK_MINUTES)
        };

        // 1. Planned Downtime. Windows are already on the grid (see `new`), so a
        //    slot is either fully planned or not planned at all.
        let mut kinds: Vec<Option<BlockKind>> = (0..n)
            .map(|i| {
                let (s, e) = slot(i);
                let best = self
                    .planned
                    .iter()
                    .map(|w| (overlap(s, e, w.start, w.end), w.kind))
                    .max_by_key(|(o, _)| *o)?;
                let planned: u32 = self
                    .planned
                    .iter()
                    .map(|w| overlap(s, e, w.start, w.end))
                    .sum();
                (planned > 0).then_some(BlockKind::Planned(best.1))
            })
            .collect();

        // 2. Lost blocks: each stop takes `lost_blocks(duration)` slots, choosing
        //    the free slots it overlaps most (FR-016, FR-017).
        for ev in &self.downtime {
            let Some((s, e)) = self.effective(*ev) else {
                continue;
            };
            let needed = lost_blocks(e - s) as usize;
            if needed == 0 {
                continue;
            }
            let mut candidates: Vec<(u32, u32, usize)> = (0..n)
                .filter(|&i| kinds[i].is_none())
                .map(|i| {
                    let (bs, be) = slot(i);
                    let o = overlap(bs, be, s, e);
                    let distance = if o > 0 {
                        0
                    } else {
                        bs.abs_diff(s).min(be.abs_diff(e))
                    };
                    (o, distance, i)
                })
                .collect();
            candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            for &(_, _, i) in candidates.iter().take(needed) {
                kinds[i] = Some(BlockKind::Lost { category: None });
            }
        }

        // 3. Everything else is Work. Apply edits.
        (0..n)
            .map(|i| {
                let (s, e) = slot(i);
                let edit = self.edits.get(&s);
                let kind = match kinds[i] {
                    None => BlockKind::Work {
                        rework: edit.is_some_and(|x| x.rework),
                    },
                    Some(BlockKind::Lost { .. }) => BlockKind::Lost {
                        category: edit.and_then(|x| x.category),
                    },
                    Some(k) => k,
                };
                Block {
                    start: s,
                    end: e,
                    kind,
                    note: edit.and_then(|x| x.note.clone()),
                }
            })
            .filter(|b| b.end <= until)
            .collect()
    }

    fn block_at(&self, start: u32) -> Result<Block, EditError> {
        self.blocks()
            .into_iter()
            .find(|b| b.start == start)
            .ok_or(EditError::NoSuchBlock)
    }

    /// Set the category of a lost block (FR-012).
    pub fn categorise(
        &mut self,
        block_start: u32,
        category: LostCategory,
    ) -> Result<(), EditError> {
        match self.block_at(block_start)?.kind {
            BlockKind::Lost { .. } => {
                self.edits.entry(block_start).or_default().category = Some(category);
                Ok(())
            }
            _ => Err(EditError::NotLostTime),
        }
    }

    /// Mark or unmark a Work block as Rework (FR-019).
    pub fn set_rework(&mut self, block_start: u32, rework: bool) -> Result<(), EditError> {
        match self.block_at(block_start)?.kind {
            BlockKind::Work { .. } => {
                self.edits.entry(block_start).or_default().rework = rework;
                Ok(())
            }
            _ => Err(EditError::NotWork),
        }
    }

    /// Set or clear a block's note (FR-025).
    pub fn set_note(&mut self, block_start: u32, note: Option<String>) -> Result<(), EditError> {
        self.block_at(block_start)?;
        self.edits.entry(block_start).or_default().note = note.filter(|n| !n.trim().is_empty());
        Ok(())
    }

    /// Start minutes of lost blocks still waiting for a category (FR-026).
    pub fn uncategorised(&self) -> Vec<u32> {
        self.blocks()
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Lost { category: None }))
            .map(|b| b.start)
            .collect()
    }

    /// FR-027: at end of day, anything still uncategorised becomes System Down.
    pub fn finalise(&mut self) {
        for start in self.uncategorised() {
            self.edits.entry(start).or_default().category = Some(LostCategory::SystemDown);
        }
        self.finalised = true;
    }

    /// Totals for the OEE maths.
    pub fn inputs(&self, settings: &Settings) -> DayInputs {
        self.inputs_until(settings, self.end)
    }

    /// Totals for a day still in progress (FR-044, FR-048): day minutes, planned
    /// minutes and blocks are clipped to `until`. The target stays the full
    /// day's target, so Efficiency reads as progress towards it.
    pub fn inputs_until(&self, settings: &Settings, until: u32) -> DayInputs {
        let mut losses = LossMinutes::default();
        let mut work_points = 0;
        let mut rework_points = 0;
        for b in self.blocks_until(until) {
            match b.kind {
                BlockKind::Work { rework } => {
                    work_points += 1;
                    rework_points += u32::from(rework);
                }
                BlockKind::Lost { .. } => {
                    let c = b.effective_category().expect("lost block");
                    losses.add(c, c.downtime_minutes());
                }
                BlockKind::Planned(_) => {}
            }
        }
        DayInputs {
            day_minutes: self.end.min(until).saturating_sub(self.start),
            planned_minutes: self.planned_minutes_until(until),
            losses,
            work_points,
            rework_points,
            target_points: settings.target_points(self.start, self.end),
        }
    }
}

/// Snap a planned window outwards to whole 20-minute blocks from `day_start`.
fn snap(mut w: PlannedWindow, day_start: u32) -> PlannedWindow {
    let down = |t: u32| {
        if t <= day_start {
            t
        } else {
            day_start + (t - day_start) / BLOCK_MINUTES * BLOCK_MINUTES
        }
    };
    let up = |t: u32| {
        if t <= day_start {
            t
        } else {
            day_start + (t - day_start).div_ceil(BLOCK_MINUTES) * BLOCK_MINUTES
        }
    };
    w.start = down(w.start);
    w.end = up(w.end);
    w
}

fn overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> u32 {
    a_end.min(b_end).saturating_sub(a_start.max(b_start))
}
