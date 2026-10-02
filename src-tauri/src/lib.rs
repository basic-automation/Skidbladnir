//! The Tauri shell.
//!
//! This layer is deliberately thin: it owns the window and the IPC boundary and nothing
//! else. Every decision about encoding lives in `skidbladnir-encode`, which has no Tauri
//! dependency and is therefore testable without a window. If logic starts accumulating
//! here, it belongs in a crate under `crates/` instead.

mod commands;
pub mod legal;
pub mod preferences;
pub mod presets;
pub mod preview;
mod updater;

/// Run the application: the gpui window, unless it is built without the `gpui` feature or
/// `SKIDBLADNIR_UI=webview` asks for the webview window.
pub fn run() {
	#[cfg(feature = "gpui")]
	if std::env::var_os("SKIDBLADNIR_UI").is_none_or(|ui| ui != "webview") {
		skidbladnir_gpui::run(skidbladnir_gpui::Options { version: env!("CARGO_PKG_VERSION"), installed_as: installed_as() });
		return;
	}
	run_webview();
}

/// The package tauri-bundler stamped this binary as, for the gpui window's updater.
#[cfg(feature = "gpui")]
fn installed_as() -> Option<skidbladnir_gpui::updater::Install> {
	use skidbladnir_gpui::updater::Install;
	use tauri::utils::{config::BundleType, platform::bundle_type};
	match bundle_type()? {
		BundleType::AppImage => Some(Install::AppImage),
		BundleType::Deb => Some(Install::Deb),
		BundleType::Rpm => Some(Install::Rpm),
		BundleType::Nsis => Some(Install::Nsis),
		BundleType::Msi => Some(Install::Msi),
		BundleType::App => Some(Install::MacApp),
		BundleType::Dmg => None,
	}
}

/// Build and run the webview window.
///
/// # Panics
///
/// Panics if Tauri cannot create the window, which is not a recoverable condition: there
/// is no useful headless mode for an image-conversion GUI.
pub fn run_webview() {
	tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_updater::Builder::new().build()).manage(commands::CancelFlag::default()).manage(updater::PendingUpdate::default()).invoke_handler(tauri::generate_handler![commands::encoder_version, commands::edition, commands::default_settings, commands::validate_settings, commands::webp_apply_preset, commands::webp_apply_lossless_level, commands::webp_cwebp_defaults, commands::convert_image, commands::inspect_dropped_paths, commands::load_preferences, commands::save_preferences, commands::cancel_conversion, commands::list_presets, commands::save_preset, commands::delete_preset, commands::preview_encode, commands::legal_document, commands::scan_folder, commands::plan_outputs, commands::convert_scanned, updater::check_for_update, updater::install_update]).run(tauri::generate_context!()).expect("Skidbladnir failed to start");
}
