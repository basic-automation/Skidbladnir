//! Named settings presets the user saves themselves.
//!
//! Distinct from libwebp's built-in `-preset` values, which are encoder parameter sets
//! chosen by libwebp. These are the user's own: "my product shots", "screenshots for the
//! wiki". The Electron app had no equivalent.
//!
//! All presets live in **one** JSON file keyed by name rather than a file per preset. That
//! is deliberate: a name typed by the user must never become a path component, or
//! `../../something` becomes a way to write outside the config directory.

use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};
use skidbladnir_encode::settings::EncodeJob;
use thiserror::Error;

/// The file inside the app's config directory.
const FILE_NAME: &str = "presets.json";

/// The longest a preset name may be. Long enough for a sentence, short enough that the UI
/// can show it and that a pasted file cannot be used to bloat the config.
const MAX_NAME: usize = 80;

/// A named settings preset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
	/// What the user called it.
	pub name: String,
	/// The settings it restores.
	pub settings: EncodeJob,
}

/// Why a preset could not be saved.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PresetError {
	/// The name was empty or only whitespace.
	#[error("a preset needs a name")]
	EmptyName,
	/// The name was longer than [`MAX_NAME`].
	#[error("a preset name can be at most {MAX_NAME} characters")]
	NameTooLong,
	/// The settings would not encode.
	#[error("these settings are not valid: {0}")]
	InvalidSettings(#[from] skidbladnir_encode::settings::ValidationError),
	/// The file could not be written.
	#[error("could not save presets: {0}")]
	Write(String),
}

/// Read the saved presets, sorted by name.
///
/// Returns an empty list rather than an error for a missing or damaged file: presets are a
/// convenience and must not stop the app opening. A damaged file is left on disk rather
/// than deleted, so nothing is destroyed behind the user's back — the next successful save
/// replaces it.
#[must_use]
pub fn load_from(directory: &Path) -> Vec<Preset> {
	let Ok(text) = fs::read_to_string(directory.join(FILE_NAME)) else { return Vec::new() };
	let Ok(stored) = serde_json::from_str::<BTreeMap<String, EncodeJob>>(&text) else { return Vec::new() };
	// A BTreeMap is already name-ordered, which keeps the UI's list stable across saves.
	// Anything the encoder would refuse is dropped rather than offered to the user.
	stored.into_iter().filter(|(_, settings)| settings.validate().is_ok()).map(|(name, settings)| Preset { name, settings }).collect()
}

/// Save a preset under `name`, replacing any existing one with that name.
///
/// # Errors
///
/// Returns [`PresetError`] if the name is empty or too long, the settings are invalid, or
/// the file cannot be written.
pub fn save_to(directory: &Path, name: &str, settings: &EncodeJob) -> Result<Vec<Preset>, PresetError> {
	let name = name.trim();
	if name.is_empty() {
		return Err(PresetError::EmptyName);
	}
	if name.chars().count() > MAX_NAME {
		return Err(PresetError::NameTooLong);
	}
	settings.validate()?;

	let mut stored: BTreeMap<String, EncodeJob> = load_from(directory).into_iter().map(|preset| (preset.name, preset.settings)).collect();
	stored.insert(name.to_owned(), settings.clone());
	write(directory, &stored)?;
	Ok(load_from(directory))
}

/// Remove the preset called `name`. Removing one that does not exist is not an error.
///
/// # Errors
///
/// Returns [`PresetError::Write`] if the file cannot be written.
pub fn delete_from(directory: &Path, name: &str) -> Result<Vec<Preset>, PresetError> {
	let mut stored: BTreeMap<String, EncodeJob> = load_from(directory).into_iter().map(|preset| (preset.name, preset.settings)).collect();
	stored.remove(name.trim());
	write(directory, &stored)?;
	Ok(load_from(directory))
}

/// Write the whole set, staged and renamed like every other file this app writes.
fn write(directory: &Path, stored: &BTreeMap<String, EncodeJob>) -> Result<(), PresetError> {
	let to_error = |error: io::Error| PresetError::Write(error.to_string());
	fs::create_dir_all(directory).map_err(to_error)?;
	let text = serde_json::to_string_pretty(stored).map_err(|error| PresetError::Write(error.to_string()))?;
	let staging = directory.join(format!(".{FILE_NAME}.{}.part", std::process::id()));
	fs::write(&staging, text).map_err(to_error)?;
	if let Err(error) = fs::rename(&staging, directory.join(FILE_NAME)) {
		let _ = fs::remove_file(&staging);
		return Err(to_error(error));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf};

	use skidbladnir_encode::settings::{EncodeJob, Mode, WebpSettings};

	use super::{PresetError, delete_from, load_from, save_to};

	struct Scratch(PathBuf);

	impl Scratch {
		fn new(name: &str) -> Self {
			let path = std::env::temp_dir().join(format!("skidbladnir-presets-{}-{name}", std::process::id()));
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
	fn saves_lists_and_deletes() {
		let scratch = Scratch::new("crud");
		assert_eq!(load_from(&scratch.0), Vec::new(), "nothing saved yet");

		let shots = EncodeJob::from(WebpSettings { quality: 88, ..Default::default() });
		let presets = save_to(&scratch.0, "Product shots", &shots).expect("save");
		assert_eq!(presets.len(), 1);
		assert_eq!(presets[0].name, "Product shots");
		assert_eq!(presets[0].settings.webp.quality, 88);

		let presets = save_to(&scratch.0, "Wiki screenshots", &EncodeJob::from(WebpSettings { mode: Mode::Lossless, ..Default::default() })).expect("save");
		// Sorted by name, so the list does not reshuffle between saves.
		assert_eq!(presets.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Product shots", "Wiki screenshots"]);

		let presets = delete_from(&scratch.0, "Product shots").expect("delete");
		assert_eq!(presets.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Wiki screenshots"]);
	}

	#[test]
	fn saving_the_same_name_replaces_rather_than_duplicates() {
		let scratch = Scratch::new("replace");
		save_to(&scratch.0, "Mine", &EncodeJob::from(WebpSettings { quality: 10, ..Default::default() })).expect("save");
		let presets = save_to(&scratch.0, "Mine", &EncodeJob::from(WebpSettings { quality: 90, ..Default::default() })).expect("save again");
		assert_eq!(presets.len(), 1);
		assert_eq!(presets[0].settings.webp.quality, 90);
	}

	#[test]
	fn names_are_trimmed_and_must_not_be_empty() {
		let scratch = Scratch::new("names");
		assert_eq!(save_to(&scratch.0, "   ", &EncodeJob::default()), Err(PresetError::EmptyName));
		assert_eq!(save_to(&scratch.0, "", &EncodeJob::default()), Err(PresetError::EmptyName));
		let presets = save_to(&scratch.0, "  Padded  ", &EncodeJob::default()).expect("save");
		assert_eq!(presets[0].name, "Padded");
	}

	#[test]
	fn absurd_names_are_refused() {
		let scratch = Scratch::new("longname");
		assert_eq!(save_to(&scratch.0, &"x".repeat(81), &EncodeJob::default()), Err(PresetError::NameTooLong));
		assert!(save_to(&scratch.0, &"x".repeat(80), &EncodeJob::default()).is_ok());
	}

	/// A name is a JSON key, never a path component. This is the test that says so.
	#[test]
	fn a_name_that_looks_like_a_path_stays_inside_the_config_directory() {
		let scratch = Scratch::new("traversal");
		let nasty = "../../escaped";
		save_to(&scratch.0, nasty, &EncodeJob::default()).expect("save");
		assert_eq!(load_from(&scratch.0).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec![nasty]);
		// Exactly one file, and it is the presets file.
		let entries: Vec<String> = fs::read_dir(&scratch.0).expect("list").filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
		assert_eq!(entries, vec!["presets.json".to_owned()], "a preset name must not create files: {entries:?}");
		assert!(!scratch.0.parent().expect("parent").join("escaped").exists(), "nothing may be written outside the directory");
	}

	#[test]
	fn invalid_settings_are_refused() {
		let scratch = Scratch::new("invalid");
		let error = save_to(&scratch.0, "Bad", &EncodeJob::from(WebpSettings { segments: 9, ..Default::default() })).expect_err("must refuse");
		assert!(matches!(error, PresetError::InvalidSettings(_)), "got {error}");
		assert_eq!(load_from(&scratch.0), Vec::new(), "nothing should have been written");
	}

	/// A damaged file must not stop the app, and must not be silently deleted either.
	#[test]
	fn a_damaged_file_reads_as_empty_and_is_left_alone() {
		let scratch = Scratch::new("damaged");
		let path = scratch.0.join("presets.json");
		fs::write(&path, b"{ not json").expect("write");
		assert_eq!(load_from(&scratch.0), Vec::new());
		assert!(path.exists(), "a damaged file must be left on disk, not destroyed");
	}

	/// A stored preset that the encoder would now reject is hidden rather than offered.
	#[test]
	fn stored_presets_that_are_invalid_are_not_offered() {
		let scratch = Scratch::new("staleinvalid");
		fs::write(scratch.0.join("presets.json"), br#"{"Good":{"quality":50},"Bad":{"method":42}}"#).expect("write");
		let presets = load_from(&scratch.0);
		assert_eq!(presets.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["Good"]);
	}

	#[test]
	fn deleting_something_absent_is_not_an_error() {
		let scratch = Scratch::new("deleteabsent");
		assert_eq!(delete_from(&scratch.0, "nope").expect("delete"), Vec::new());
	}
}
