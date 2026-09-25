//! The IPC surface.
//!
//! Each command is a thin wrapper over `skidbladnir-encode` so that the interesting
//! behaviour stays testable without a window. Errors cross the boundary as strings
//! because that is all the frontend can act on; the typed error stays on this side.

use std::path::PathBuf;

use serde::Serialize;
use skidbladnir_encode::{
	encoder::{linked_decoder_version, linked_encoder_version}, settings::EncodeSettings, source::{Conversion, encode_file, output_path_in}
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
pub fn default_settings() -> EncodeSettings {
	EncodeSettings::default()
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
pub fn validate_settings(settings: EncodeSettings) -> Result<EncodeSettings, String> {
	match settings.validate() {
		Ok(()) => Ok(settings),
		Err(error) => Err(error.to_string()),
	}
}

/// The result of a conversion, as the UI needs it: what was written, and the before and
/// after sizes the Electron app also reported and users relied on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
}

/// Convert one image into `output_directory`, using the Electron app's output naming.
///
/// The destination is a *directory* rather than a file path, matching how the UI asks the
/// user for it, and the filename is derived the same way the Electron app derives it so
/// converted files land where people already expect.
///
/// Refusing to overwrite the source is enforced in the encode core rather than here, so
/// no caller can skip it.
///
/// # Errors
///
/// Returns the failure as a string for display.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes command arguments into owned values; the encode core borrows them")]
pub fn convert_image(settings: EncodeSettings, input: PathBuf, output_directory: PathBuf) -> Result<ConversionReport, String> {
	let output_path = output_path_in(output_directory, &input);
	let conversion = encode_file(&settings, &input, &output_path).map_err(|error| error.to_string())?;
	Ok(ConversionReport { input_path: input, output_path, conversion })
}

#[cfg(test)]
mod tests {
	use skidbladnir_encode::settings::{EncodeSettings, Mode};

	use super::{convert_image, default_settings, encoder_version, validate_settings};

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
		assert_eq!(default_settings(), EncodeSettings::default());
	}

	#[test]
	fn validate_settings_reports_the_same_rules_as_the_encoder() {
		assert_eq!(validate_settings(EncodeSettings::default()), Ok(EncodeSettings::default()));
		let missing_preset = EncodeSettings { mode: Mode::Preset, preset: None, ..Default::default() };
		assert_eq!(validate_settings(missing_preset), Err("mode is `preset` but no preset was selected".to_owned()));
		let bad_method = EncodeSettings { method: 9, ..Default::default() };
		assert_eq!(validate_settings(bad_method), Err("method is 9, but must be in 0..=6".to_owned()));
	}

	/// A frontend that sends a partial payload gets the filled-in settings back, so it can
	/// show what the encoder will actually do rather than guessing at the defaults.
	#[test]
	fn validate_settings_returns_the_filled_in_form() {
		let partial: EncodeSettings = serde_json::from_str(r#"{"mode":"lossless","quality":90}"#).expect("partial settings deserialize");
		let validated = validate_settings(partial).expect("partial settings are valid");
		assert_eq!(validated.quality, 90);
		assert_eq!(validated.method, EncodeSettings::default().method, "an omitted field comes back as the core's default");
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
		let bytes = skidbladnir_encode::encoder::encode_rgba(&EncodeSettings { mode: Mode::Lossless, ..Default::default() }, &skidbladnir_encode::encoder::RgbaImage { width: 1, height: 1, pixels: &pixels }).expect("encode the fixture");
		std::fs::write(&webp, &bytes).expect("write the fixture");

		let error = convert_image(EncodeSettings::default(), webp.clone(), dir.clone()).expect_err("must refuse");
		assert!(error.contains("refusing to overwrite the source"), "got {error}");
		assert_eq!(std::fs::read(&webp).expect("read it back"), bytes, "the source must be untouched");

		let _ = std::fs::remove_dir_all(&dir);
	}

	/// The settings must survive the JSON round trip the IPC boundary actually performs.
	#[test]
	fn settings_cross_the_ipc_boundary_intact() {
		let settings = default_settings();
		let json = serde_json::to_string(&settings).expect("serialize");
		let returned: EncodeSettings = serde_json::from_str(&json).expect("deserialize");
		assert_eq!(returned, settings);
		assert_eq!(validate_settings(returned), Ok(settings));
	}
}
