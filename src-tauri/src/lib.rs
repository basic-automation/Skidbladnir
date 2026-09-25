//! The Tauri shell.
//!
//! This layer is deliberately thin: it owns the window and the IPC boundary and nothing
//! else. Every decision about encoding lives in `skidbladnir-encode`, which has no Tauri
//! dependency and is therefore testable without a window. If logic starts accumulating
//! here, it belongs in a crate under `crates/` instead.

mod commands;

/// Build and run the application.
///
/// # Panics
///
/// Panics if Tauri cannot create the window, which is not a recoverable condition: there
/// is no useful headless mode for an image-conversion GUI.
pub fn run() {
	tauri::Builder::default().plugin(tauri_plugin_dialog::init()).invoke_handler(tauri::generate_handler![commands::encoder_version, commands::default_settings, commands::validate_settings, commands::convert_image]).run(tauri::generate_context!()).expect("Skidbladnir failed to start");
}
