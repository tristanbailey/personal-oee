//! Reopen the window when the Dock icon is clicked (D-47).
//!
//! winit's app delegate doesn't answer macOS's "reopen" request, so clicking
//! the Dock icon while the window is hidden does nothing. On macOS we add
//! `applicationShouldHandleReopen:hasVisibleWindows:` to the delegate's class.
//! It only sets a flag and wakes the UI; `App::update` shows the window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use eframe::egui;

static REQUESTED: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<egui::Context> = OnceLock::new();

/// True once per Dock click since the last call.
pub fn take_requested() -> bool {
    REQUESTED.swap(false, Ordering::SeqCst)
}

/// Whether a Dock click should bring the window back: only if it is hidden.
pub fn should_show(requested: bool, window_visible: bool) -> bool {
    requested && !window_visible
}

#[cfg(target_os = "macos")]
pub fn install(ctx: egui::Context) -> Result<(), String> {
    use objc2::ffi::class_addMethod;
    use objc2::runtime::{AnyObject, Bool, Sel};
    use objc2::{msg_send, msg_send_id, sel};
    use objc2::rc::Retained;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    extern "C" fn should_handle_reopen(
        _this: *mut AnyObject,
        _sel: Sel,
        _sender: *mut AnyObject,
        _has_visible_windows: Bool,
    ) -> Bool {
        REQUESTED.store(true, Ordering::SeqCst);
        if let Some(ctx) = CTX.get() {
            ctx.request_repaint();
        }
        // We show the window ourselves; skip AppKit's default handling.
        Bool::NO
    }

    let _ = CTX.set(ctx);
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let delegate: Option<Retained<AnyObject>> = unsafe { msg_send_id![&app, delegate] };
    let delegate = delegate.ok_or("the app has no delegate yet")?;
    let class: *const objc2::runtime::AnyClass = unsafe { msg_send![&delegate, class] };
    // BOOL is "B" on Apple silicon and "c" on Intel.
    #[cfg(target_arch = "aarch64")]
    let types = c"B@:@B";
    #[cfg(not(target_arch = "aarch64"))]
    let types = c"c@:@c";
    let imp: unsafe extern "C" fn() = unsafe {
        std::mem::transmute::<
            extern "C" fn(*mut AnyObject, Sel, *mut AnyObject, Bool) -> Bool,
            unsafe extern "C" fn(),
        >(should_handle_reopen)
    };
    let added = unsafe {
        class_addMethod(
            class as *mut _,
            sel!(applicationShouldHandleReopen:hasVisibleWindows:).as_ptr(),
            Some(imp),
            types.as_ptr(),
        )
    };
    if Bool::from_raw(added).as_bool() {
        Ok(())
    } else {
        Err("the app delegate already handles reopen".into())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install(_ctx: egui::Context) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dock_click_only_shows_a_hidden_window() {
        assert!(should_show(true, false));
        assert!(!should_show(true, true));
        assert!(!should_show(false, false));
    }
}
