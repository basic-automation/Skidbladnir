//! The macOS window's frame. gpui opens a window with no titlebar as titled but not
//! resizable: its edges do not resize it, and gpui has no client-side resizing on macOS
//! (`start_window_resize` does nothing there). Tauri's frameless window resizes from its
//! edges, so this adds what gpui leaves out.

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindowButton, NSWindowStyleMask};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Let the window be resized from its edges, and hide the traffic lights: the window draws
/// its own minimise, maximise and close, and the titled style keeps `AppKit`'s greyed out
/// over the rail (and, once resizable, a live green one).
pub fn make_resizable(window: &impl HasWindowHandle) {
	let Ok(handle) = window.window_handle() else { return };
	let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
	// SAFETY: gpui's AppKit handle is its content view, alive as long as the window; this
	// runs on the main thread, in the closure that opens the window.
	let Some(view): Option<Retained<NSView>> = (unsafe { Retained::retain(handle.ns_view.as_ptr().cast()) }) else { return };
	let Some(ns_window) = view.window() else { return };
	ns_window.setStyleMask(ns_window.styleMask() | NSWindowStyleMask::Resizable);
	for button in [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton] {
		if let Some(button) = ns_window.standardWindowButton(button) {
			button.setHidden(true);
		}
	}
}
