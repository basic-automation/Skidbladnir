//! In-app updates.
//!
//! Before this, a user who installed a release stayed on it until they happened to visit the
//! releases page: the five years between v0.4.3 and v0.5.0 were five years nobody's copy
//! changed. The window now asks, once per launch, whether a newer release exists, and offers
//! it; nothing is downloaded or installed until the user says so.
//!
//! The check and the install both happen here rather than through the updater plugin's own
//! IPC, so the window gains exactly two commands and no plugin permissions — the same
//! reason the capability file grants no filesystem access.
//!
//! # Where updates come from
//!
//! `tauri.conf.json` points the plugin at `latest.json` on the repository's `updater`
//! branch, which `release.yml` rewrites after every release. GitHub's own
//! `releases/latest` cannot be used: every release is a prerelease until the migration
//! ships, and `latest` skips prereleases. Every download is checked against the minisign
//! public key in the same file, so a manifest that pointed somewhere else would fail
//! verification rather than install.

use std::sync::Mutex;

use serde::Serialize;
use tauri::Emitter as _;
use tauri_plugin_updater::{Update, UpdaterExt as _};

/// Set to anything but the empty string to skip the check. The `WebDriver` harness sets it so a smoke test never
/// depends on the network or on whatever release happens to be current.
const DISABLE_ENV: &str = "SKIDBLADNIR_NO_UPDATE_CHECK";

/// The update found by the last check, held until the user accepts it.
///
/// Kept on this side because an [`Update`] carries the verified download URL and signature;
/// the window only ever sees [`UpdateInfo`], so it cannot ask for anything other than what
/// the check found.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

/// What the window shows about an available update.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
	/// The version on offer.
	pub version: String,
	/// The version running now.
	pub current_version: String,
}

/// Download progress, emitted as `update-progress`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
	/// Bytes received so far.
	downloaded: u64,
	/// The size of the download, when the server says.
	total: Option<u64>,
}

/// Whether this build should look for updates at all.
///
/// Debug builds never do: they are the dev server's window, and offering to replace them
/// with a release would be wrong. Neither does a run with [`DISABLE_ENV`] set.
fn checks_enabled() -> bool {
	!cfg!(debug_assertions) && std::env::var_os(DISABLE_ENV).is_none_or(|value| value.is_empty())
}

/// Ask the update server whether a newer release exists.
///
/// Resolves to `None` when there is nothing newer, and also when checking is disabled.
///
/// # Errors
///
/// Returns why the check failed — offline, no manifest yet, or no build for this platform
/// in it. The window treats every one of those as "no update" and says nothing: an update
/// check is never worth an error on screen at launch.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle, state: tauri::State<'_, PendingUpdate>) -> Result<Option<UpdateInfo>, String> {
	if !checks_enabled() {
		return Ok(None);
	}
	let update = app.updater().map_err(|error| error.to_string())?.check().await.map_err(|error| error.to_string())?;
	let info = update.as_ref().map(|update| UpdateInfo { version: update.version.clone(), current_version: update.current_version.clone() });
	*state.0.lock().map_err(|_| "the pending update is unavailable".to_owned())? = update;
	Ok(info)
}

/// Download, verify and install the update the last check found, then restart into it.
///
/// On Windows the installer takes over and the process exits inside this call; elsewhere
/// the new version is in place when the download finishes and the app restarts itself.
///
/// # Errors
///
/// Returns why the update could not be applied: there was none pending, the download
/// failed, the signature did not verify, or the installer refused. The running copy is
/// untouched in every one of those cases.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, state: tauri::State<'_, PendingUpdate>) -> Result<(), String> {
	// Cloned, not taken: if the download fails the user can try again without a new check.
	let update = state.0.lock().map_err(|_| "the pending update is unavailable".to_owned())?.clone().ok_or("there is no update to install")?;
	let mut downloaded: u64 = 0;
	update.download_and_install(
		|chunk, total| {
			downloaded += chunk as u64;
			let _ = app.emit("update-progress", UpdateProgress { downloaded, total });
		},
		|| {},
	)
	.await
	.map_err(|error| error.to_string())?;
	app.restart()
}
