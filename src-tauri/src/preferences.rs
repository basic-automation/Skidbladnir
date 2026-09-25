//! Remembering the user's settings between launches.
//!
//! The Electron app forgot everything on every run, so a user who worked at quality 92
//! with sharp YUV on re-entered it each time. This stores the last-used settings and
//! destination in the app's config directory.
//!
//! # Why this is careful rather than three lines of `serde_json`
//!
//! A preferences file is read at startup, and it is the one file the user can plausibly
//! hand-edit or have half-written by a crash. Neither may prevent the app from opening:
//! anything unreadable, unparseable **or invalid** falls back to the defaults, and the
//! caller is told which happened so the UI can say so rather than silently discarding
//! what someone typed. Writes are staged and renamed, so a crash mid-save leaves the
//! previous preferences intact instead of a truncated file.

use std::{
	fs, io, path::{Path, PathBuf}
};

use serde::{Deserialize, Serialize};
use skidbladnir_encode::settings::EncodeSettings;

/// The file inside the app's config directory.
const FILE_NAME: &str = "preferences.json";

/// What the app remembers between launches.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
	/// The encode settings last used.
	pub settings: EncodeSettings,
	/// The destination directory last chosen, if it still exists.
	pub output_directory: Option<PathBuf>,
}

/// Why a preferences file was not used, when it existed but could not be honoured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreferencesFallback {
	/// There was no file; this is the first launch.
	NoFile,
	/// The file could not be read.
	Unreadable,
	/// The file was not valid JSON for these preferences.
	Unparseable,
	/// The file parsed but held settings the encoder would reject.
	InvalidSettings,
}

/// Preferences as loaded, plus what happened if the stored ones could not be used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedPreferences {
	/// The preferences to use.
	pub preferences: Preferences,
	/// `None` when the stored preferences were used as-is.
	pub fell_back: Option<PreferencesFallback>,
}

/// Load preferences from `directory`, falling back to defaults rather than failing.
///
/// Never returns an error: the app must open. The reason for any fallback is reported so
/// the UI can tell the user their saved settings were discarded and why.
#[must_use]
pub fn load_from(directory: &Path) -> LoadedPreferences {
	let path = directory.join(FILE_NAME);
	let defaults = || Preferences::default();

	let Ok(text) = fs::read_to_string(&path) else {
		let reason = if path.exists() { PreferencesFallback::Unreadable } else { PreferencesFallback::NoFile };
		return LoadedPreferences { preferences: defaults(), fell_back: Some(reason) };
	};

	let Ok(preferences) = serde_json::from_str::<Preferences>(&text) else {
		return LoadedPreferences { preferences: defaults(), fell_back: Some(PreferencesFallback::Unparseable) };
	};

	// A hand-edited or future-version file can parse and still hold values the encoder
	// will refuse. Catching that here means the UI opens on something usable instead of
	// failing at the first conversion.
	if preferences.settings.validate().is_err() {
		return LoadedPreferences { preferences: defaults(), fell_back: Some(PreferencesFallback::InvalidSettings) };
	}

	// A destination that has since been deleted or unmounted is dropped rather than
	// presented as still selected.
	let preferences = Preferences { output_directory: preferences.output_directory.filter(|dir| dir.is_dir()), ..preferences };

	LoadedPreferences { preferences, fell_back: None }
}

/// Write preferences into `directory`, creating it if needed.
///
/// Staged through a temporary file and renamed, so a crash mid-write cannot leave a
/// truncated preferences file that the next launch then discards.
///
/// # Errors
///
/// Returns the I/O error if the directory cannot be created or the file cannot be written.
pub fn save_to(directory: &Path, preferences: &Preferences) -> io::Result<()> {
	fs::create_dir_all(directory)?;
	let staging = directory.join(format!(".{FILE_NAME}.{}.part", std::process::id()));
	let text = serde_json::to_string_pretty(preferences).map_err(io::Error::other)?;
	fs::write(&staging, text)?;
	if let Err(error) = fs::rename(&staging, directory.join(FILE_NAME)) {
		let _ = fs::remove_file(&staging);
		return Err(error);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf};

	use skidbladnir_encode::settings::{EncodeSettings, Mode};

	use super::{Preferences, PreferencesFallback, load_from, save_to};

	/// A scratch directory that cleans itself up. Nothing here touches a path outside the
	/// temp directory.
	struct Scratch(PathBuf);

	impl Scratch {
		fn new(name: &str) -> Self {
			let path = std::env::temp_dir().join(format!("skidbladnir-prefs-{}-{name}", std::process::id()));
			let _ = fs::remove_dir_all(&path);
			fs::create_dir_all(&path).expect("create the scratch directory");
			Self(path)
		}
	}

	impl Drop for Scratch {
		fn drop(&mut self) {
			let _ = fs::remove_dir_all(&self.0);
		}
	}

	#[test]
	fn round_trips_settings_and_destination() {
		let scratch = Scratch::new("roundtrip");
		let preferences = Preferences { settings: EncodeSettings { mode: Mode::Lossless, quality: 92, sharp_yuv: true, ..Default::default() }, output_directory: Some(scratch.0.clone()) };
		save_to(&scratch.0, &preferences).expect("save");
		let loaded = load_from(&scratch.0);
		assert_eq!(loaded.fell_back, None);
		assert_eq!(loaded.preferences, preferences);
	}

	#[test]
	fn a_first_launch_gets_the_defaults() {
		let scratch = Scratch::new("firstlaunch");
		let loaded = load_from(&scratch.0);
		assert_eq!(loaded.fell_back, Some(PreferencesFallback::NoFile));
		assert_eq!(loaded.preferences, Preferences::default());
		assert_eq!(loaded.preferences.settings, EncodeSettings::default());
	}

	/// A truncated file — the shape a crash mid-write would leave — must not stop the app
	/// opening.
	#[test]
	fn a_corrupt_file_falls_back_and_says_so() {
		let scratch = Scratch::new("corrupt");
		fs::write(scratch.0.join("preferences.json"), b"{\"settings\": {\"quality\": 9").expect("write the truncated file");
		let loaded = load_from(&scratch.0);
		assert_eq!(loaded.fell_back, Some(PreferencesFallback::Unparseable));
		assert_eq!(loaded.preferences.settings, EncodeSettings::default());
	}

	/// A file that parses but holds values the encoder rejects must not be loaded, or the
	/// app opens in a state that fails at the first conversion.
	#[test]
	fn settings_the_encoder_would_reject_are_not_loaded() {
		let scratch = Scratch::new("invalid");
		fs::write(scratch.0.join("preferences.json"), br#"{"settings":{"method":99}}"#).expect("write the invalid file");
		let loaded = load_from(&scratch.0);
		assert_eq!(loaded.fell_back, Some(PreferencesFallback::InvalidSettings));
		assert_eq!(loaded.preferences.settings.method, EncodeSettings::default().method);
	}

	/// A partial file is a normal thing for a version upgrade to produce; missing fields
	/// take the core's defaults rather than failing the whole load.
	#[test]
	fn a_partial_file_fills_in_from_the_defaults() {
		let scratch = Scratch::new("partial");
		fs::write(scratch.0.join("preferences.json"), br#"{"settings":{"quality":33}}"#).expect("write the partial file");
		let loaded = load_from(&scratch.0);
		assert_eq!(loaded.fell_back, None);
		assert_eq!(loaded.preferences.settings.quality, 33);
		assert_eq!(loaded.preferences.settings.segments, EncodeSettings::default().segments);
	}

	/// A destination that has been deleted or unmounted since last launch is dropped.
	#[test]
	fn a_destination_that_no_longer_exists_is_dropped() {
		let scratch = Scratch::new("gonedir");
		let gone = scratch.0.join("removable");
		fs::create_dir_all(&gone).expect("create it");
		save_to(&scratch.0, &Preferences { output_directory: Some(gone.clone()), ..Default::default() }).expect("save");
		fs::remove_dir_all(&gone).expect("remove it");
		assert_eq!(load_from(&scratch.0).preferences.output_directory, None);
	}

	#[test]
	fn saving_leaves_no_staging_file_behind() {
		let scratch = Scratch::new("staging");
		save_to(&scratch.0, &Preferences::default()).expect("save");
		let leftovers: Vec<_> = fs::read_dir(&scratch.0).expect("list").filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().contains(".part")).collect();
		assert!(leftovers.is_empty(), "{leftovers:?}");
	}

	#[test]
	fn saving_creates_the_directory() {
		let scratch = Scratch::new("mkdir");
		let nested = scratch.0.join("config").join("skidbladnir");
		save_to(&nested, &Preferences::default()).expect("save into a directory that does not exist yet");
		assert!(nested.join("preferences.json").is_file());
	}
}
