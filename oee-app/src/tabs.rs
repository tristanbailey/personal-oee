//! The three top-level screens and which one was open last (D-29).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Today,
    Week,
    Calendar,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Today, Tab::Week, Tab::Calendar];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Today => "Today",
            Tab::Week => "Week",
            Tab::Calendar => "Calendar",
        }
    }
}

/// Remembered between launches, in `ui.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiState {
    #[serde(default)]
    pub tab: Tab,
}
