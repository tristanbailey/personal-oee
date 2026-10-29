//! macOS notifications for the engine's notices (D-42, FR-026).
//!
//! Each notice becomes a notification. Clicking it asks the window to come to
//! the front. Sending waits for the click, so it runs on its own thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;
use oee_core::{Notice, PlannedKind};

use crate::reminder::blocks_need;

/// The title and body for a notice.
pub fn message_for(notice: &Notice) -> (String, String) {
    match notice {
        Notice::PlannedStarted { kind, label } => {
            let kind = match kind {
                PlannedKind::Break => "Break",
                PlannedKind::Meeting => "Meeting",
            };
            let name = if label.is_empty() { kind } else { label };
            (
                "Planned downtime started".into(),
                format!("{name}. Back to Work when it ends."),
            )
        }
        Notice::PlannedEnded => ("Back to work".into(), "Planned downtime has ended.".into()),
        Notice::CategoriseReminder { blocks } => (
            "Lost time needs a category".into(),
            blocks_need(*blocks),
        ),
        Notice::DayEnded => (
            "Day ended".into(),
            "Open Personal OEE to finish the day.".into(),
        ),
    }
}

/// True when running from inside an `.app` bundle, where notifications can be
/// attributed to the app itself.
pub fn is_bundled(exe: &std::path::Path) -> bool {
    exe.to_string_lossy().contains(".app/Contents/MacOS/")
}

pub const BUNDLE_ID: &str = "com.tristanbailey.personaloee";

#[derive(Clone)]
pub struct Notifier {
    ctx: egui::Context,
    open: Arc<AtomicBool>,
    errors: Arc<Mutex<Vec<String>>>,
}

impl Notifier {
    pub fn new(ctx: egui::Context) -> Self {
        #[cfg(target_os = "macos")]
        if std::env::current_exe().is_ok_and(|p| is_bundled(&p)) {
            // Only possible once; without a bundle macOS attributes the
            // notification to a default app instead.
            let _ = mac_notification_sys::set_application(BUNDLE_ID);
        }
        Self {
            ctx,
            open: Arc::new(AtomicBool::new(false)),
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Show a notification. Returns straight away.
    pub fn send(&self, notice: &Notice) {
        let (title, body) = message_for(notice);
        let this = self.clone();
        std::thread::spawn(move || this.deliver(&title, &body));
    }

    #[cfg(target_os = "macos")]
    fn deliver(&self, title: &str, body: &str) {
        use mac_notification_sys::{Notification, NotificationResponse};
        let result = Notification::new()
            .title(title)
            .message(body)
            .default_sound()
            .wait_for_click(true)
            .send();
        match result {
            Ok(NotificationResponse::Click) => {
                self.open.store(true, Ordering::SeqCst);
                self.ctx.request_repaint();
            }
            Ok(_) => {}
            Err(e) => {
                self.errors
                    .lock()
                    .unwrap()
                    .push(format!("Couldn't show a notification: {e}"));
                self.ctx.request_repaint();
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn deliver(&self, _title: &str, _body: &str) {}

    /// True once per click on a notification.
    pub fn take_open(&self) -> bool {
        self.open.swap(false, Ordering::SeqCst)
    }

    /// Delivery problems since the last call.
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.lock().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn each_notice_has_a_title_and_body() {
        let started = Notice::PlannedStarted {
            kind: PlannedKind::Break,
            label: "Lunch".into(),
        };
        assert_eq!(
            message_for(&started),
            (
                "Planned downtime started".to_string(),
                "Lunch. Back to Work when it ends.".to_string()
            )
        );
        let unnamed = Notice::PlannedStarted {
            kind: PlannedKind::Meeting,
            label: String::new(),
        };
        assert_eq!(message_for(&unnamed).1, "Meeting. Back to Work when it ends.");
        assert_eq!(
            message_for(&Notice::PlannedEnded),
            ("Back to work".to_string(), "Planned downtime has ended.".to_string())
        );
        assert_eq!(
            message_for(&Notice::CategoriseReminder { blocks: 2 }),
            (
                "Lost time needs a category".to_string(),
                "2 lost blocks need a category.".to_string()
            )
        );
        assert_eq!(
            message_for(&Notice::CategoriseReminder { blocks: 1 }).1,
            "1 lost block needs a category."
        );
        assert_eq!(message_for(&Notice::DayEnded).0, "Day ended");
    }

    #[test]
    fn bundled_apps_are_recognised_by_their_path() {
        assert!(is_bundled(Path::new(
            "/Applications/PersonalOEE.app/Contents/MacOS/PersonalOEE"
        )));
        assert!(!is_bundled(Path::new("/Users/x/target/release/PersonalOEE")));
    }
}
