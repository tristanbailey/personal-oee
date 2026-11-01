//! Personal OEE storage and app-level logic, with no UI.
//!
//! Shared by the desktop app (`oee-app`) and the command-line tool, so both
//! read and write the same files and apply the same rules.
//!
//! - [`store`]: saved days, calendar, settings and UI state under one folder.
//! - [`settings`]: day defaults, the downtime shortcut and the weekly goal.
//! - [`start`]: time parsing and the Not started screen's rules.
//! - [`tabs`]: the screens and which one was open last.
//! - [`export_range`]: which dates to export, and writing the CSV files.

pub mod export_range;
pub mod settings;
pub mod start;
pub mod store;
pub mod tabs;
