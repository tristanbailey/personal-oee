//! The system-wide downtime shortcut (D-40, FR-011a, NFR-002).
//!
//! ⌃⌥D toggles downtime from any app. The OS reports the key on its own thread,
//! so the press is parked in a flag and the window is woken to read it.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use eframe::egui;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

/// Keeps the registration alive. Dropping it releases the shortcut.
pub struct Hotkey {
    manager: GlobalHotKeyManager,
    current: HotKey,
    /// The id the event handler listens for, so the key can change.
    id: Arc<AtomicU32>,
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
        let id = Arc::new(AtomicU32::new(hotkey.id()));
        let listen = id.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == listen.load(Ordering::SeqCst) && event.state == HotKeyState::Pressed {
                flag.store(true, Ordering::SeqCst);
                // Wake the window even when it's in the background.
                ctx.request_repaint();
            }
        }));
        Ok(Hotkey {
            manager,
            current: hotkey,
            id,
            pressed,
        })
    }

    /// Switch to a new shortcut (D-44). The old one stays if the new one
    /// can't be registered, for example because another app has it.
    pub fn change(&mut self, hotkey: HotKey) -> Result<(), String> {
        if hotkey == self.current {
            return Ok(());
        }
        self.manager.register(hotkey).map_err(|e| format!("{e}"))?;
        let _ = self.manager.unregister(self.current);
        self.id.store(hotkey.id(), Ordering::SeqCst);
        self.current = hotkey;
        Ok(())
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
    fn d_in_the_window_only_works_when_nothing_else_wants_the_key() {
        assert!(d_key_toggles(true, false, false, false));
        assert!(!d_key_toggles(false, false, false, false));
        assert!(!d_key_toggles(true, true, false, false)); // ⌘D etc.
        assert!(!d_key_toggles(true, false, true, false)); // typing a note
        assert!(!d_key_toggles(true, false, false, true)); // editor open
    }
}
