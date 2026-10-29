//! The system-wide downtime shortcut (D-40, FR-011a, NFR-002).
//!
//! ⌃⌥D toggles downtime from any app. The OS reports the key on its own thread,
//! so the press is parked in a flag and the window is woken to read it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// The default shortcut: Control + Option + D. Changeable in settings (D-44).
pub fn default_hotkey() -> HotKey {
    HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD)
}

pub const DEFAULT_LABEL: &str = "⌃⌥D";

/// Keeps the registration alive. Dropping it releases the shortcut.
pub struct Hotkey {
    _manager: GlobalHotKeyManager,
    pressed: Arc<AtomicBool>,
}

impl Hotkey {
    /// Register `hotkey`. Must be called on the main thread. The error is
    /// plain text for the Messages panel, for example if another app has the
    /// shortcut.
    pub fn register(ctx: egui::Context, hotkey: HotKey) -> Result<Hotkey, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| format!("{e}"))?;
        manager.register(hotkey).map_err(|e| format!("{e}"))?;
        let pressed = Arc::new(AtomicBool::new(false));
        let flag = pressed.clone();
        let id = hotkey.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == id && event.state == HotKeyState::Pressed {
                flag.store(true, Ordering::SeqCst);
                // Wake the window even when it's in the background.
                ctx.request_repaint();
            }
        }));
        Ok(Hotkey {
            _manager: manager,
            pressed,
        })
    }

    /// True once per press.
    pub fn take_pressed(&self) -> bool {
        self.pressed.swap(false, Ordering::SeqCst)
    }
}

/// D inside the window also toggles downtime, unless the user is typing, a
/// modifier is held, or a dialog is open.
pub fn d_key_toggles(d_pressed: bool, any_modifier: bool, typing: bool, dialog_open: bool) -> bool {
    d_pressed && !any_modifier && !typing && !dialog_open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_control_option_d() {
        let key = default_hotkey();
        assert_eq!(key, HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyD));
        assert_eq!(DEFAULT_LABEL, "⌃⌥D");
    }

    #[test]
    fn d_in_the_window_only_works_when_nothing_else_wants_the_key() {
        assert!(d_key_toggles(true, false, false, false));
        assert!(!d_key_toggles(false, false, false, false));
        assert!(!d_key_toggles(true, true, false, false)); // ⌘D etc.
        assert!(!d_key_toggles(true, false, true, false)); // typing a note
        assert!(!d_key_toggles(true, false, false, true)); // editor open
    }
}
