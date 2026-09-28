// The window is the product, so on Windows the console that would otherwise open behind
// it is suppressed in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
	// WebKitGTK's DMA-BUF renderer kills the window at startup on some Wayland compositors
	// and GPU drivers ("Error 71 (Protocol error) dispatching to Wayland display"); seen on
	// Hyprland. Turning it off costs nothing visible for a settings window. A value the user
	// set themselves is left alone.
	#[cfg(target_os = "linux")]
	if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
		// SAFETY: this is the first thing `main` does, before Tauri, GTK or anything else
		// has started a thread that could be reading the environment.
		unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
	}

	skidbladnir_lib::run();
}
