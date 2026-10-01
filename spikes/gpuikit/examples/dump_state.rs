//! Print what the Tauri app's startup commands return on this machine, as JSON, for the
//! browser stand-in of the old window (`load_preferences`, `list_presets`, and `preview_encode` for an image if one is named).

#[path = "../src/shell/mod.rs"]
mod shell;

fn main() {
	let directory = shell::config_directory();
	let state = serde_json::json!({
		"loadPreferences": shell::preferences::load_from(&directory),
		"listPresets": shell::presets::load_from(&directory),
		"defaultSettings": skidbladnir_encode::EncodeJob::default(),
		// `preview_encode` for the image named on the command line, at the stored settings.
		"preview": std::env::args_os().nth(1).map(|image| shell::preview::preview(&shell::preferences::load_from(&directory).preferences.settings, std::path::Path::new(&image))),
	});
	println!("{state}");
}
