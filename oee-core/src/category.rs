//! Lost-time and Planned Downtime categories.

use serde::{Deserialize, Serialize};

use std::ops::AddAssign;

use crate::rules::BLOCK_MINUTES;

/// FR-009, FR-024: the five fixed lost-time categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LostCategory {
    SystemDown,
    ChangeOver,
    Idle,
    Management,
    Wait,
}

impl LostCategory {
    pub const ALL: [LostCategory; 5] = [
        Self::SystemDown,
        Self::ChangeOver,
        Self::Idle,
        Self::Management,
        Self::Wait,
    ];

    /// Downtime minutes one block of this category counts for in Availability.
    /// FR-015: Change Over counts as 10 minutes, though it displays as 20.
    pub fn downtime_minutes(self) -> u32 {
        match self {
            Self::ChangeOver => BLOCK_MINUTES / 2,
            _ => BLOCK_MINUTES,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SystemDown => "System Down",
            Self::ChangeOver => "Change Over",
            Self::Idle => "Idle",
            Self::Management => "Management",
            Self::Wait => "Wait",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::SystemDown => "The computer or system isn't accessible, or work isn't possible",
            Self::ChangeOver => "A slight loss of time from context switching between tasks",
            Self::Idle => "Doing a non-work task, or taking an unscheduled break",
            Self::Management => "Managing others, or in an unplanned meeting to discuss work",
            Self::Wait => "Waiting for other people or a dependency to be ready",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// FR-031: Planned Downtime types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlannedKind {
    Break,
    Meeting,
}

/// Lost minutes per category, for Availability and the loss breakdown (FR-042).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LossMinutes([u32; 5]);

impl LossMinutes {
    pub fn new(system_down: u32, change_over: u32, idle: u32, management: u32, wait: u32) -> Self {
        Self([system_down, change_over, idle, management, wait])
    }

    pub fn get(&self, category: LostCategory) -> u32 {
        self.0[category.index()]
    }

    pub fn add(&mut self, category: LostCategory, minutes: u32) {
        self.0[category.index()] += minutes;
    }

    /// Total downtime minutes across all categories.
    pub fn total(&self) -> u32 {
        self.0.iter().sum()
    }

    /// `(category, minutes)` pairs in display order.
    pub fn iter(&self) -> impl Iterator<Item = (LostCategory, u32)> + '_ {
        LostCategory::ALL.iter().map(|&c| (c, self.get(c)))
    }
}

impl AddAssign for LossMinutes {
    fn add_assign(&mut self, other: Self) {
        for (a, b) in self.0.iter_mut().zip(other.0) {
            *a += b;
        }
    }
}
