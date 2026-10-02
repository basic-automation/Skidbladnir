//! The "Choose images…" dialog with the old window's filter.
//!
//! gpui's `prompt_for_paths` has no file-type filters, so it lists every file. The Tauri window
//! offers one filter, "Images", with the extensions below (`index.vue`, `chooseInputs`). On
//! Linux this asks the desktop's file-chooser portal directly, through the same `ashpd` gpui
//! itself uses for its dialogs; elsewhere it uses `rfd`, which is what tauri-plugin-dialog uses.

use std::path::PathBuf;

/// The extensions the old dialog accepts.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jpe", "jif", "jfif", "jfi", "tif", "tiff", "webp", "avif", "jxl", "heic", "heif", "gif", "pnm", "pgm", "ppm", "pam", "pfm"];

/// Ask for one or more images. `None` when the dialog was cancelled or failed.
#[cfg(target_os = "linux")]
pub async fn pick_images() -> Option<Vec<PathBuf>> {
	use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
	// Globs are case-sensitive in the portal, so each extension is offered in both cases, as
	// rfd's GTK backend does for the Tauri window.
	let filter = IMAGE_EXTENSIONS.iter().fold(FileFilter::new("Images"), |filter, extension| filter.glob(&format!("*.{extension}")).glob(&format!("*.{}", extension.to_uppercase())));
	let request = SelectedFiles::open_file().title("Open File").modal(true).multiple(true).filter(filter).send().await.inspect_err(|error| eprintln!("the file chooser portal failed: {error}")).ok()?;
	// A cancelled dialog answers with a response error; that is not worth reporting.
	let files = request.response().ok()?;
	Some(files.uris().iter().filter_map(|uri| url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()).collect())
}

/// Ask for one or more images. `None` when the dialog was cancelled.
#[cfg(not(target_os = "linux"))]
pub async fn pick_images() -> Option<Vec<PathBuf>> {
	let files = rfd::AsyncFileDialog::new().add_filter("Images", IMAGE_EXTENSIONS).pick_files().await?;
	Some(files.iter().map(|file| file.path().to_path_buf()).collect())
}
