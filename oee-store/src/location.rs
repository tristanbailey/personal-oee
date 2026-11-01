//! Where the data lives (C-02): the app's real folder or a test folder.
//!
//! Commands that write must say which. A test folder comes from `--data-dir`
//! or the `OEE_DATA_DIR` variable; the real folder needs `--real`. Reads with
//! neither use the real folder, so looking never needs a flag.

use std::path::{Path, PathBuf};

pub const ENV_VAR: &str = "OEE_DATA_DIR";

/// What the caller asked for. `env` is the value of `OEE_DATA_DIR`, if set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Request {
    pub data_dir: Option<PathBuf>,
    pub real: bool,
    pub env: Option<PathBuf>,
    /// `HOME`, for the real folder.
    pub home: Option<PathBuf>,
    pub writes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationError {
    /// `--data-dir` and `--real` together, or `--real` with `OEE_DATA_DIR` set.
    Conflict,
    /// A write with no `--data-dir`, `OEE_DATA_DIR` or `--real`.
    NeedsChoice,
    /// The real folder was wanted but `HOME` isn't set.
    NoHome,
}

impl LocationError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Conflict => "location_conflict",
            Self::NeedsChoice => "data_location_required",
            Self::NoHome => "no_home",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Conflict => "choose one data location: --data-dir <path> (or OEE_DATA_DIR) for a test folder, or --real for the app's folder, not both",
            Self::NeedsChoice => "this command writes data, so say where: --data-dir <path> for a test folder, or --real for the app's real folder",
            Self::NoHome => "HOME isn't set, so the real data folder can't be found",
        }
    }
}

impl std::fmt::Display for LocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for LocationError {}

/// `~/Library/Application Support/PersonalOEE`.
pub fn real_folder(home: &Path) -> PathBuf {
    home.join("Library/Application Support/PersonalOEE")
}

/// The folder to use. An empty `env` counts as unset.
pub fn resolve(req: &Request) -> Result<PathBuf, LocationError> {
    let env = req.env.as_ref().filter(|p| !p.as_os_str().is_empty());
    if req.real && (req.data_dir.is_some() || env.is_some()) {
        return Err(LocationError::Conflict);
    }
    if let Some(dir) = req.data_dir.as_ref().or(env) {
        return Ok(dir.clone());
    }
    if req.writes && !req.real {
        return Err(LocationError::NeedsChoice);
    }
    req.home.as_deref().map(real_folder).ok_or(LocationError::NoHome)
}

/// Read the environment: `OEE_DATA_DIR` and `HOME`.
pub fn from_environment(data_dir: Option<PathBuf>, real: bool, writes: bool) -> Request {
    Request {
        data_dir,
        real,
        env: std::env::var_os(ENV_VAR).map(PathBuf::from),
        home: std::env::var_os("HOME").map(PathBuf::from),
        writes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> Request {
        Request {
            home: Some("/Users/t".into()),
            ..Request::default()
        }
    }

    #[test]
    fn reads_default_to_the_real_folder() {
        assert_eq!(
            resolve(&req()),
            Ok(PathBuf::from("/Users/t/Library/Application Support/PersonalOEE"))
        );
    }

    #[test]
    fn writes_with_no_choice_are_refused() {
        let r = Request { writes: true, ..req() };
        assert_eq!(resolve(&r), Err(LocationError::NeedsChoice));
        assert_eq!(LocationError::NeedsChoice.code(), "data_location_required");
    }

    #[test]
    fn a_flag_or_the_variable_picks_a_test_folder() {
        let flag = Request { data_dir: Some("/tmp/a".into()), writes: true, ..req() };
        assert_eq!(resolve(&flag), Ok("/tmp/a".into()));
        let env = Request { env: Some("/tmp/b".into()), writes: true, ..req() };
        assert_eq!(resolve(&env), Ok("/tmp/b".into()));
        // The flag beats the variable.
        let both = Request { data_dir: Some("/tmp/a".into()), env: Some("/tmp/b".into()), ..req() };
        assert_eq!(resolve(&both), Ok("/tmp/a".into()));
    }

    #[test]
    fn real_picks_the_app_folder_for_writes() {
        let r = Request { real: true, writes: true, ..req() };
        assert_eq!(
            resolve(&r),
            Ok("/Users/t/Library/Application Support/PersonalOEE".into())
        );
    }

    #[test]
    fn real_with_a_test_folder_is_a_conflict() {
        let flag = Request { real: true, data_dir: Some("/tmp/a".into()), ..req() };
        assert_eq!(resolve(&flag), Err(LocationError::Conflict));
        let env = Request { real: true, env: Some("/tmp/b".into()), ..req() };
        assert_eq!(resolve(&env), Err(LocationError::Conflict));
    }

    #[test]
    fn an_empty_variable_counts_as_unset() {
        let r = Request { env: Some("".into()), writes: true, ..req() };
        assert_eq!(resolve(&r), Err(LocationError::NeedsChoice));
    }

    #[test]
    fn the_real_folder_needs_home() {
        let r = Request { real: true, ..Request::default() };
        assert_eq!(resolve(&r), Err(LocationError::NoHome));
    }
}
