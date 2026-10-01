//! The Tauri shell's own storage code, compiled in unchanged with `#[path]` so the spike
//! reads and writes the same files, in the same shape, as the app it is compared against.

#[path = "../../../../src-tauri/src/preferences.rs"]
pub mod preferences;
#[path = "../../../../src-tauri/src/presets.rs"]
pub mod presets;
#[path = "../../../../src-tauri/src/preview.rs"]
pub mod preview;

use std::path::PathBuf;

/// Where Tauri keeps this app's config: `app_config_dir()` for the bundle identifier.
#[must_use]
pub fn config_directory() -> PathBuf {
	let base = std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"), PathBuf::from);
	base.join("com.basicautomation.skidbladnir")
}
