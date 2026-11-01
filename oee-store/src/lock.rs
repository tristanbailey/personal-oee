//! The app's lock file (C-11). While the desktop app is open it holds
//! `<data-dir>/.lock`, containing its process id. The command-line tool
//! refuses to write there (exit 6) unless told `--force`, because the app
//! keeps today's day in memory and would overwrite the change.
//!
//! A lock left behind by a crash names a process that is gone, and is ignored.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const FILE: &str = ".lock";

fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// Held by the running app. Dropping it removes the file, if it is still ours.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
    pid: u32,
}

impl Lock {
    pub fn acquire(root: &Path) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        let pid = std::process::id();
        let path = path(root);
        fs::write(&path, pid.to_string())?;
        Ok(Self { path, pid })
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path).ok().and_then(|t| t.trim().parse::<u32>().ok()) == Some(self.pid) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// The process id of another running app that holds the lock, if any.
pub fn held_by_other(root: &Path) -> Option<u32> {
    let pid: u32 = fs::read_to_string(path(root)).ok()?.trim().parse().ok()?;
    (pid != std::process::id() && is_running(pid)).then_some(pid)
}

#[cfg(unix)]
fn is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn is_running(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oee-lock-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn our_own_lock_does_not_block_us_and_is_removed_on_drop() {
        let d = dir("own");
        let lock = Lock::acquire(&d).unwrap();
        assert!(d.join(FILE).exists());
        assert_eq!(held_by_other(&d), None);
        drop(lock);
        assert!(!d.join(FILE).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_running_other_process_blocks() {
        let d = dir("other");
        fs::create_dir_all(&d).unwrap();
        // The parent shell (or test runner) is running and is not us.
        let parent = std::os::unix::process::parent_id();
        fs::write(d.join(FILE), parent.to_string()).unwrap();
        assert_eq!(held_by_other(&d), Some(parent));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_stale_or_unreadable_lock_is_ignored() {
        let d = dir("stale");
        fs::create_dir_all(&d).unwrap();
        // Pids this large are not in use.
        fs::write(d.join(FILE), "4000000").unwrap();
        assert_eq!(held_by_other(&d), None);
        fs::write(d.join(FILE), "not a number").unwrap();
        assert_eq!(held_by_other(&d), None);
        assert_eq!(held_by_other(&d.join("missing")), None);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn dropping_does_not_remove_someone_elses_lock() {
        let d = dir("theirs");
        let lock = Lock::acquire(&d).unwrap();
        fs::write(d.join(FILE), "1").unwrap();
        drop(lock);
        assert!(d.join(FILE).exists());
        let _ = fs::remove_dir_all(&d);
    }
}
