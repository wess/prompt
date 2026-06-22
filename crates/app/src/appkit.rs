//! macOS AppKit reach-through for the quick terminal. gpui exposes no
//! window-level or visibility controls, but it implements `HasWindowHandle`,
//! so we borrow the live `NSWindow` and message it directly to float the
//! window above every app and Space and to hide/show it without destroying
//! the session.

use gpui::Window;

/// Turn `window` into an overlay: above other applications, present on every
/// Space, and drawn over fullscreen apps. Idempotent.
pub fn make_overlay(window: &Window) {
    imp::make_overlay(window);
}

/// Order the window in and make it key, preserving its session.
pub fn show(window: &Window) {
    imp::show(window);
}

/// Order the window out (hidden but alive), preserving its session.
pub fn hide(window: &Window) {
    imp::hide(window);
}

/// Whether the window is currently on screen.
pub fn is_visible(window: &Window) -> bool {
    imp::is_visible(window)
}

#[cfg(target_os = "macos")]
mod imp {
    use gpui::Window;
    use objc2_app_kit::{
        NSStatusWindowLevel, NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn make_overlay(window: &Window) {
        with_nswindow(window, |w| {
            w.setLevel(NSStatusWindowLevel);
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::Stationary,
            );
            // A dropdown terminal has no window chrome: hide the traffic
            // lights so there is no close affordance (dismiss via the hotkey,
            // double-Escape, or `exit`).
            for button in [
                NSWindowButton::CloseButton,
                NSWindowButton::MiniaturizeButton,
                NSWindowButton::ZoomButton,
            ] {
                if let Some(button) = w.standardWindowButton(button) {
                    button.setHidden(true);
                }
            }
        });
    }

    pub fn show(window: &Window) {
        with_nswindow(window, |w| w.makeKeyAndOrderFront(None));
    }

    pub fn hide(window: &Window) {
        with_nswindow(window, |w| w.orderOut(None));
    }

    pub fn is_visible(window: &Window) -> bool {
        let mut visible = false;
        with_nswindow(window, |w| visible = w.isVisible());
        visible
    }

    /// Run `f` with the window's `NSWindow`, if its native handle is live.
    /// Must be called on the main thread (gpui guarantees this for the
    /// `handle.update`/render closures we call it from).
    fn with_nswindow(window: &Window, f: impl FnOnce(&NSWindow)) {
        // `Window` has an inherent `window_handle()` returning gpui's own
        // handle, so reach the raw-window-handle trait method by UFCS.
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(h) = handle.as_raw() else {
            return;
        };
        // SAFETY: gpui hands us a valid, retained NSView pointer for the
        // lifetime of the window; we only borrow it for this call.
        let view: &NSView = unsafe { &*(h.ns_view.as_ptr() as *const NSView) };
        if let Some(nswindow) = view.window() {
            f(&nswindow);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use gpui::Window;

    pub fn make_overlay(_window: &Window) {}
    pub fn show(_window: &Window) {}
    pub fn hide(_window: &Window) {}
    pub fn is_visible(_window: &Window) -> bool {
        true
    }
}
