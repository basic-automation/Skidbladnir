//! The IPC surface.
//!
//! Each command is a thin wrapper over `skidbladnir-encode` so that the interesting
//! behaviour stays testable without a window. Errors cross the boundary as strings
//! because that is all the frontend can act on; the typed error stays on this side.

use std::{
	path::{Path, PathBuf}, sync::{
		Arc, atomic::{AtomicBool, Ordering}
	}
};

use serde::Serialize;
use skidbladnir_encode::{
	encoder::{linked_decoder_version, linked_encoder_version}, settings::EncodeJob, source::{Conversion, FoundImage, PathInspection, encode_file_with_progress, inspect_paths, mirrored_output_path, output_path_in, scan_directory}
};
use tauri::{Emitter as _, Manager as _};

use crate::{
	preferences::{self, LoadedPreferences, Preferences}, presets::{self, Preset}, preview::{self, Preview}
};

/// A human-readable description of the encode core, for the UI's about/diagnostics view.
///
/// It reports the libwebp the binary is actually linked against rather than a version
/// written down somewhere, which is the only form of the answer worth showing: encoder
/// output depends on it.
#[tauri::command]
#[must_use]
pub fn encoder_version() -> String {
	let (emajor, eminor, erevision) = linked_encoder_version();
	let (dmajor, dminor, drevision) = linked_decoder_version();
	format!("Skidbladnir {} · libwebp encoder {emajor}.{eminor}.{erevision} · decoder {dmajor}.{dminor}.{drevision}", env!("CARGO_PKG_VERSION"))
}

/// The settings a freshly opened window starts from.
///
/// The frontend does not carry its own copy of the defaults: they are the Electron UI's
/// own numbers, they are pinned by test in the encode core, and duplicating them in
/// TypeScript is how the two drift apart.
#[tauri::command]
#[must_use]
pub fn default_settings() -> EncodeJob {
	EncodeJob::default()
}

/// Check settings without encoding anything, so the UI can disable its convert button
/// against the same rules the encoder enforces rather than a re-implementation of them.
///
/// Returns the settings back on success. That is not ceremony: the frontend may have
/// omitted fields, and what comes back is the filled-in form the encoder would actually
/// use, so the UI can display exactly what it is about to do.
///
/// # Errors
///
/// Returns the validation failure as a string for display.
#[tauri::command]
pub fn validate_settings(settings: EncodeJob) -> Result<EncodeJob, String> {
	match settings.validate() {
		Ok(()) => Ok(settings),
		Err(error) => Err(error.to_string()),
	}
}

/// The result of a conversion, as the UI needs it: what was written, and the before and
/// after sizes the Electron app also reported and users relied on.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionReport {
	/// Which file was converted. Echoed back because a batch UI dispatches many
	/// conversions at once and needs to attach each result to its row.
	pub input_path: PathBuf,
	/// Where the converted file was written.
	pub output_path: PathBuf,
	/// Sizes and dimensions.
	#[serde(flatten)]
	pub conversion: Conversion,
	/// [`Conversion::saving_percent`], computed here so the window displays the core's
	/// figure instead of recomputing it in TypeScript, where the two could disagree at the
	/// rounding edges. `null` for a zero-byte source.
	pub saving_percent: Option<f64>,
}

impl ConversionReport {
	/// A report for `conversion`, with the saving filled in from it.
	#[must_use]
	pub fn new(input_path: PathBuf, output_path: PathBuf, conversion: Conversion) -> Self {
		Self { input_path, output_path, conversion, saving_percent: conversion.saving_percent() }
	}
}

/// Identify which of these paths Skidbladnir can actually read.
///
/// Used by the drag-and-drop handler so the window can accept a mixed selection and say
/// what it will do with it. The answer comes from the encode core's own content sniffing,
/// not from a list of extensions in the frontend, so the UI can never accept a file the
/// loader will then reject.
#[tauri::command]
#[must_use]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes command arguments into owned values; the encode core borrows them")]
pub fn inspect_dropped_paths(paths: Vec<PathBuf>) -> Vec<PathInspection> {
	inspect_paths(&paths)
}

/// Where preferences live: the OS's own per-app config location.
///
/// Falls back to the current directory only if Tauri cannot resolve one, which should not
/// happen on a desktop platform; preferences are a convenience, so a failure here must not
/// stop the app.
fn config_directory(app: &tauri::AppHandle) -> PathBuf {
	app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// The settings and destination the app should open with.
///
/// Never fails: anything unreadable, unparseable or invalid falls back to the defaults,
/// and `fellBack` says which so the UI can tell the user their saved settings were
/// discarded rather than silently losing them.
#[tauri::command]
#[must_use]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects AppHandle by value; there is no by-reference form of a command argument")]
pub fn load_preferences(app: tauri::AppHandle) -> LoadedPreferences {
	preferences::load_from(&config_directory(&app))
}

/// Remember these settings and destination for the next launch.
///
/// # Errors
///
/// Returns the failure as a string for display.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects AppHandle and deserializes arguments by value; the store borrows them")]
pub fn save_preferences(app: tauri::AppHandle, preferences: Preferences) -> Result<(), String> {
	preferences::save_to(&config_directory(&app), &preferences).map_err(|error| error.to_string())
}

/// Shared cancellation flag, held in Tauri's managed state.
///
/// There is no way to interrupt `WebPEncode` from outside, so cancelling means the encode
/// noticing this flag from inside its own progress callback and refusing to continue.
#[derive(Debug, Default)]
pub struct CancelFlag(pub Arc<AtomicBool>);

/// Ask the running conversion to stop.
///
/// Takes effect at the encoder's next progress callback, so it is not instantaneous — a
/// file already being written finishes or is discarded cleanly rather than being cut off
/// half-written.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects State by value; there is no by-reference form of a command argument")]
pub fn cancel_conversion(state: tauri::State<'_, CancelFlag>) {
	state.0.store(true, Ordering::Relaxed);
}

/// How far along a conversion is, emitted as the `conversion-progress` event.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionProgress {
	/// The file being converted.
	pub input_path: PathBuf,
	/// Encoder progress, 0 to 100.
	///
	/// libwebp does not guarantee a final call at 100, so this reaching 100 is not how
	/// completion is detected — the command returning is.
	pub percent: u32,
}

/// Find the images in a directory the user picked, optionally descending into it.
///
/// Symlinks are not followed and the walk is depth- and count-bounded, so dropping a home
/// directory on the window produces a bounded answer rather than hanging it.
#[tauri::command]
#[must_use]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes command arguments into owned values; the scanner borrows them")]
pub fn scan_folder(directory: PathBuf, recursive: bool) -> Vec<FoundImage> {
	scan_directory(&directory, recursive)
}

/// Convert one image found by a folder scan, mirroring its position under the scan root
/// into the output directory.
///
/// Separate from [`convert_image`] because the destination is computed differently: a
/// flat selection writes everything side by side, a folder scan reproduces the tree.
///
/// # Errors
///
/// Returns the failure as a string for display, including a refusal if the relative path
/// would write outside the chosen output directory.
#[tauri::command]
pub async fn convert_scanned(app: tauri::AppHandle, state: tauri::State<'_, CancelFlag>, settings: EncodeJob, input: PathBuf, relative: PathBuf, output_root: PathBuf) -> Result<ConversionReport, String> {
	let Some(output_path) = mirrored_output_path(&output_root, &relative, settings.format) else {
		return Err(format!("refusing to write `{}`: it would land outside the chosen folder", relative.display()));
	};
	let Some(parent) = output_path.parent().map(Path::to_path_buf) else {
		return Err("the output path has no parent directory".to_owned());
	};
	// The mirror only creates directories underneath the folder the user chose, which
	// `mirrored_output_path` has already confirmed.
	std::fs::create_dir_all(&parent).map_err(|error| format!("could not create `{}`: {error}", parent.display()))?;

	let cancelled = Arc::clone(&state.0);
	cancelled.store(false, Ordering::Relaxed);
	let reporting_path = input.clone();

	tauri::async_runtime::spawn_blocking(move || {
		let conversion = encode_file_with_progress(&settings, &input, &output_path, &mut |percent| {
			let _ = app.emit("conversion-progress", ConversionProgress { input_path: reporting_path.clone(), percent });
			!cancelled.load(Ordering::Relaxed)
		})
		.map_err(|error| error.to_string())?;
		Ok(ConversionReport::new(input, output_path, conversion))
	})
	.await
	.map_err(|error| format!("the conversion thread failed: {error}"))?
}

/// Encode `input` with these settings **into memory** and return it beside the original,
/// so the user can see the trade before committing a file to disk.
///
/// Both images come back as `data:` URLs, which means the preview needs no filesystem
/// permission in the webview and cannot show a stale file from an earlier run.
///
/// # Errors
///
/// Returns the failure as a string for display, including a refusal for images too large
/// to hold two copies of in the webview.
///
/// `async`, with the encode on a blocking thread, for the same reason as
/// [`convert_image`]: a synchronous command holds the event loop, and an AVIF encode at
/// its default speed takes long enough to freeze the window visibly.
#[tauri::command]
pub async fn preview_encode(settings: EncodeJob, input: PathBuf) -> Result<Preview, String> {
	tauri::async_runtime::spawn_blocking(move || preview::preview(&settings, &input)).await.map_err(|error| format!("the preview thread failed: {error}"))?
}

/// The user's saved presets, sorted by name.
///
/// Returns an empty list rather than an error for a missing or damaged file: presets are a
/// convenience and must not stop the window opening.
#[tauri::command]
#[must_use]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects AppHandle by value; there is no by-reference form of a command argument")]
pub fn list_presets(app: tauri::AppHandle) -> Vec<Preset> {
	presets::load_from(&config_directory(&app))
}

/// Save the current settings under `name`, replacing any preset already called that.
///
/// Returns the full list afterwards so the UI does not have to re-fetch it.
///
/// # Errors
///
/// Returns the failure as a string for display.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects AppHandle and deserializes arguments by value; the store borrows them")]
pub fn save_preset(app: tauri::AppHandle, name: String, settings: EncodeJob) -> Result<Vec<Preset>, String> {
	presets::save_to(&config_directory(&app), &name, &settings).map_err(|error| error.to_string())
}

/// Delete the preset called `name`. Deleting one that is not there is not an error.
///
/// # Errors
///
/// Returns the failure as a string for display.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri injects AppHandle and deserializes arguments by value; the store borrows them")]
pub fn delete_preset(app: tauri::AppHandle, name: String) -> Result<Vec<Preset>, String> {
	presets::delete_from(&config_directory(&app), &name).map_err(|error| error.to_string())
}

/// Convert one image, reporting progress through `on_progress`, which returns `false` to
/// stop.
///
/// Separate from the command so the behaviour can be tested without a Tauri `State` or an
/// `AppHandle` — the command below is a thin wrapper that supplies the event emitter and
/// the cancellation flag.
///
/// # Errors
///
/// Returns the failure as a string for display.
pub fn convert_one(settings: &EncodeJob, input: PathBuf, output_directory: PathBuf, on_progress: &mut dyn FnMut(u32) -> bool) -> Result<ConversionReport, String> {
	let output_path = output_path_in(output_directory, &input, settings.format);
	let conversion = encode_file_with_progress(settings, &input, &output_path, on_progress).map_err(|error| error.to_string())?;
	Ok(ConversionReport::new(input, output_path, conversion))
}

/// Convert one image into `output_directory`, using the Electron app's output naming.
///
/// The destination is a *directory* rather than a file path, matching how the UI asks the
/// user for it, and the filename is derived the same way the Electron app derives it so
/// converted files land where people already expect.
///
/// Refusing to overwrite the source is enforced in the encode core rather than here, so no
/// caller can skip it.
///
/// # Why this is `async` with the work on a blocking thread
///
/// A **synchronous** Tauri command blocks the event loop for as long as it runs, which for
/// a large image is seconds. That does not merely make the window unresponsive: it makes
/// cancellation impossible, because `cancel_conversion` can never be delivered while the
/// encode holds the thread. Measured, not assumed — with the synchronous version, a
/// cancel issued from the frontend on the first progress event never arrived and a
/// 1800x1400 encode ran to completion every time. Moving the encode to a blocking thread
/// and awaiting it keeps the loop free, so both progress events and the cancel get through.
///
/// # Errors
///
/// Returns the failure as a string for display, including a cancellation.
#[tauri::command]
pub async fn convert_image(app: tauri::AppHandle, state: tauri::State<'_, CancelFlag>, settings: EncodeJob, input: PathBuf, output_directory: PathBuf) -> Result<ConversionReport, String> {
	// Each file starts uncancelled, so a cancel left over from a previous batch cannot kill
	// the next one.
	let cancelled = Arc::clone(&state.0);
	cancelled.store(false, Ordering::Relaxed);
	let reporting_path = input.clone();

	tauri::async_runtime::spawn_blocking(move || {
		convert_one(&settings, input, output_directory, &mut |percent| {
			// Emitting is best-effort: a failed event must not abort a conversion the user
			// asked for.
			let _ = app.emit("conversion-progress", ConversionProgress { input_path: reporting_path.clone(), percent });
			!cancelled.load(Ordering::Relaxed)
		})
	})
	.await
	.map_err(|error| format!("the conversion thread failed: {error}"))?
}

#[cfg(test)]
mod tests {
	use skidbladnir_encode::{
		settings::{EncodeJob, Mode, WebpSettings}, source::Conversion
	};

	use super::{ConversionReport, convert_one, default_settings, encoder_version, inspect_dropped_paths, validate_settings};

	/// The version string is shown to users, so it must actually contain versions rather
	/// than a placeholder.
	#[test]
	fn encoder_version_reports_the_linked_libwebp() {
		let reported = encoder_version();
		assert!(reported.contains("libwebp encoder 1."), "got {reported}");
		assert!(reported.contains("decoder 1."), "got {reported}");
	}

	/// The defaults the frontend receives must be the core's, not a second copy.
	#[test]
	fn default_settings_are_the_cores_defaults() {
		assert_eq!(default_settings(), EncodeJob::default());
	}

	#[test]
	fn validate_settings_reports_the_same_rules_as_the_encoder() {
		assert_eq!(validate_settings(EncodeJob::default()), Ok(EncodeJob::default()));
		let missing_preset = EncodeJob::from(WebpSettings { mode: Mode::Preset, preset: None, ..Default::default() });
		assert_eq!(validate_settings(missing_preset), Err("mode is `preset` but no preset was selected".to_owned()));
		let bad_method = EncodeJob::from(WebpSettings { method: 9, ..Default::default() });
		assert_eq!(validate_settings(bad_method), Err("method is 9, but must be in 0..=6".to_owned()));
	}

	/// A frontend that sends a partial payload gets the filled-in settings back, so it can
	/// show what the encoder will actually do rather than guessing at the defaults.
	#[test]
	fn validate_settings_returns_the_filled_in_form() {
		let partial: EncodeJob = serde_json::from_str(r#"{"mode":"lossless","quality":90}"#).expect("partial settings deserialize");
		let validated = validate_settings(partial).expect("partial settings are valid");
		assert_eq!(validated.webp.quality, 90);
		assert_eq!(validated.webp.method, EncodeJob::default().webp.method, "an omitted field comes back as the core's default");
	}

	/// The drop handler must agree with the loader, because they answer the same question.
	#[test]
	fn inspect_dropped_paths_matches_what_the_loader_accepts() {
		let dir = std::env::temp_dir().join(format!("skidbladnir-drop-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).expect("create the scratch directory");
		let webp = dir.join("image.webp");
		let pixels = [1_u8, 2, 3, 255];
		let bytes = skidbladnir_encode::encoder::encode_rgba(&EncodeJob::from(WebpSettings { mode: Mode::Lossless, ..Default::default() }), &skidbladnir_encode::encoder::RgbaImage { width: 1, height: 1, pixels: &pixels }).expect("encode the fixture");
		std::fs::write(&webp, &bytes).expect("write the fixture");
		let junk = dir.join("readme.txt");
		std::fs::write(&junk, b"not an image").expect("write the junk file");

		let inspected = inspect_dropped_paths(vec![webp.clone(), junk.clone()]);
		assert_eq!(inspected.len(), 2);
		assert!(inspected[0].supported && inspected[0].format == Some("WebP"), "{inspected:#?}");
		assert!(!inspected[1].supported, "{inspected:#?}");
		// The loader must agree on both, or the UI accepts what the encoder refuses.
		assert!(skidbladnir_encode::source::load(&webp).is_ok());
		assert!(skidbladnir_encode::source::load(&junk).is_err());

		let _ = std::fs::remove_dir_all(&dir);
	}

	/// The conversion command must reach the file-safety guard in the core rather than
	/// reimplementing or bypassing it.
	#[test]
	fn convert_image_refuses_to_overwrite_the_source() {
		let dir = std::env::temp_dir().join(format!("skidbladnir-cmd-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).expect("create the scratch directory");

		// A 1x1 lossless WebP, written by the core itself so the fixture is real.
		let webp = dir.join("only.webp");
		let pixels = [10_u8, 20, 30, 255];
		let bytes = skidbladnir_encode::encoder::encode_rgba(&EncodeJob::from(WebpSettings { mode: Mode::Lossless, ..Default::default() }), &skidbladnir_encode::encoder::RgbaImage { width: 1, height: 1, pixels: &pixels }).expect("encode the fixture");
		std::fs::write(&webp, &bytes).expect("write the fixture");

		let error = convert_one(&EncodeJob::default(), webp.clone(), dir.clone(), &mut |_| true).expect_err("must refuse");
		assert!(error.contains("refusing to overwrite the source"), "got {error}");
		assert_eq!(std::fs::read(&webp).expect("read it back"), bytes, "the source must be untouched");

		let _ = std::fs::remove_dir_all(&dir);
	}

	/// The window reads the saving from the report rather than computing its own, so the
	/// report must carry the core's figure under the name the frontend reads.
	#[test]
	fn conversion_report_carries_the_cores_saving() {
		let conversion = Conversion { source_bytes: 1000, output_bytes: 250, width: 4, height: 3 };
		let json = serde_json::to_value(ConversionReport::new("in.png".into(), "in.webp".into(), conversion)).expect("serialize");
		assert_eq!(json["savingPercent"], serde_json::json!(75.0));
		assert_eq!(json["sourceBytes"], serde_json::json!(1000), "the sizes must stay flattened alongside it");

		let empty = Conversion { source_bytes: 0, output_bytes: 10, width: 1, height: 1 };
		let json = serde_json::to_value(ConversionReport::new("in.png".into(), "in.webp".into(), empty)).expect("serialize");
		assert!(json["savingPercent"].is_null(), "a zero-byte source has no saving, not a saving of zero");
	}

	/// The settings must survive the JSON round trip the IPC boundary actually performs.
	#[test]
	fn settings_cross_the_ipc_boundary_intact() {
		let settings = default_settings();
		let json = serde_json::to_string(&settings).expect("serialize");
		let returned: EncodeJob = serde_json::from_str(&json).expect("deserialize");
		assert_eq!(returned, settings);
		assert_eq!(validate_settings(returned), Ok(settings));
	}
}
