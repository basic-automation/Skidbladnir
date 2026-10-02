//! The Tauri shell's own storage and preview code, compiled in unchanged with `#[path]`, so
//! the two windows read and write the same files in the same shape.

#[path = "../../../../src-tauri/src/preferences.rs"]
pub mod preferences;
#[path = "../../../../src-tauri/src/presets.rs"]
pub mod presets;
#[path = "../../../../src-tauri/src/preview.rs"]
pub mod preview;

use std::path::PathBuf;

/// Where Tauri keeps this app's config: `app_config_dir()`, the platform's config folder
/// and the bundle identifier.
#[must_use]
pub fn config_directory() -> PathBuf {
	dirs::config_dir().unwrap_or_default().join("com.basicautomation.skidbladnir")
}
