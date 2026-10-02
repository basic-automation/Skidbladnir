//! In-app updates from the signed manifest `release.yml` publishes, as tauri-plugin-updater
//! does them for the Tauri window (`src-tauri/src/updater.rs`).
//!
//! - **Check:** read the manifest, take this install's platform entry (the installer-specific
//!   key first, as `linux-x86_64-appimage`, then the plain `linux-x86_64`) and offer it if
//!   its version is newer.
//! - **Install:** download the artifact, verify its minisign signature against the public key
//!   in `tauri.conf.json` and refuse it if that fails, then replace the running AppImage and
//!   relaunch it.
//!
//! Only an AppImage can replace itself here. The .deb (which tauri-plugin-updater installs
//! with `pkexec dpkg`), the Windows installer and the macOS bundle are not implemented, and
//! say so rather than pretend.

use std::{
	io::Read as _, path::{Path, PathBuf}
};

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Each edition's own manifest, so a GPL install only ever updates to a GPL build: the
/// endpoint in `tauri.conf.json` for the standard edition, and the one `tauri.gpl.conf.json`
/// overrides it with for the GPL edition. `release.yml` publishes both.
const MANIFEST: &str = if skidbladnir_encode::settings::HEIC_X265 {
	"https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest-gpl.json"
} else {
	"https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest.json"
};

/// `plugins.updater.pubkey` in `tauri.conf.json`: a base64-wrapped minisign public key.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwOThEOTc3MDc2NEQ2MjEKUldRaDFtUUhkOW1ZNEtMWHE0UFNrc1FtME5lWmlKdUN3cE5ta2dKaE8wOWo0UEozUFFvNmpXYk4K";

/// An update on offer.
#[derive(Clone, Debug)]
pub struct Update {
	pub version: String,
	pub url: String,
	/// The `.sig` file's content, base64 as the manifest carries it.
	pub signature: String,
}

/// The running AppImage, when this is one: the AppImage runtime sets `APPIMAGE`.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
	std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|path| path.is_file())
}

/// The manifest's platform keys for this install, most specific first.
fn platform_keys() -> Vec<String> {
	let os = match std::env::consts::OS {
		"macos" => "darwin",
		other => other,
	};
	let base = format!("{os}-{}", std::env::consts::ARCH);
	let installer = match os {
		"linux" if appimage().is_some() => Some("appimage"),
		"linux" => Some("deb"),
		"windows" => Some("nsis"),
		"darwin" => Some("app"),
		_ => None,
	};
	installer.map(|installer| format!("{base}-{installer}")).into_iter().chain([base]).collect()
}

/// The update on offer, if the manifest has one newer than `current` for this platform.
#[must_use]
pub fn check(current: &str) -> Option<Update> {
	let body = ureq::get(MANIFEST).call().ok()?.body_mut().read_to_string().ok()?;
	let manifest: serde_json::Value = serde_json::from_str(&body).ok()?;
	let version = manifest.get("version")?.as_str()?.trim_start_matches('v').to_owned();
	if parse(&version)? <= parse(current)? {
		return None;
	}
	let platforms = manifest.get("platforms")?;
	let entry = platform_keys().iter().find_map(|key| platforms.get(key))?;
	Some(Update { version, url: entry.get("url")?.as_str()?.to_owned(), signature: entry.get("signature")?.as_str()?.to_owned() })
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
	let mut parts = version.split(['.', '-', '+']).map(str::parse::<u64>);
	Some((parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?))
}

/// Check `data` against the base64 `signature` with the app's public key, as
/// tauri-plugin-updater does.
///
/// # Errors
///
/// Says why when the key or signature cannot be read, or the signature does not match.
pub fn verify(data: &[u8], signature: &str) -> Result<(), String> {
	let text = |base64: &str| STANDARD.decode(base64).ok().and_then(|bytes| String::from_utf8(bytes).ok());
	let key = minisign_verify::PublicKey::decode(&text(PUBLIC_KEY).ok_or("the updater key is not valid base64")?).map_err(|error| format!("the updater key is not a minisign key: {error}"))?;
	let signature = minisign_verify::Signature::decode(&text(signature).ok_or("the update's signature is not valid base64")?).map_err(|error| format!("the update's signature is not a minisign signature: {error}"))?;
	key.verify(data, &signature, true).map_err(|_| "the update's signature does not match Skidbladnir's key, so it was not installed".to_owned())
}

/// Download `update`, reporting `(downloaded, total)` as it goes.
///
/// # Errors
///
/// The download's failure, as text.
pub fn download(update: &Update, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<Vec<u8>, String> {
	let mut response = ureq::get(&update.url).call().map_err(|error| format!("could not download the update: {error}"))?;
	let total = response.headers().get("content-length").and_then(|value| value.to_str().ok()?.parse().ok());
	let mut reader = response.body_mut().with_config().limit(1 << 30).reader();
	let mut data = Vec::with_capacity(usize::try_from(total.unwrap_or(0)).unwrap_or(0));
	let mut chunk = vec![0; 1 << 16];
	loop {
		let read = reader.read(&mut chunk).map_err(|error| format!("the download was interrupted: {error}"))?;
		if read == 0 {
			break;
		}
		data.extend_from_slice(&chunk[..read]);
		progress(data.len() as u64, total);
	}
	Ok(data)
}

/// Replace the AppImage at `target` with `data`, which has been verified. Written beside it
/// and renamed over it, so a failure part-way leaves the old one in place.
///
/// # Errors
///
/// The filesystem's failure, as text.
pub fn replace(target: &Path, data: &[u8]) -> Result<(), String> {
	use std::os::unix::fs::PermissionsExt as _;
	let staged = target.with_extension("AppImage.update");
	std::fs::write(&staged, data).map_err(|error| format!("could not write {}: {error}", staged.display()))?;
	std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(|error| format!("could not make {} executable: {error}", staged.display()))?;
	std::fs::rename(&staged, target).map_err(|error| {
		let _ = std::fs::remove_file(&staged);
		format!("could not replace {}: {error}", target.display())
	})
}

/// Download, verify and install `update`, then return the AppImage to relaunch.
///
/// # Errors
///
/// Why it could not, as text for the banner.
pub fn install(update: &Update, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<PathBuf, String> {
	let Some(target) = appimage() else {
		return Err("this copy is not running as an AppImage, and only an AppImage can update itself here".to_owned());
	};
	let data = download(update, progress)?;
	verify(&data, &update.signature)?;
	replace(&target, &data)?;
	Ok(target)
}

#[cfg(test)]
mod tests {
	use super::{parse, verify};

	#[test]
	fn versions_compare_numerically() {
		assert!(parse("1.10.0") > parse("1.9.9"));
		assert_eq!(parse("v1.1.0".trim_start_matches('v')), Some((1, 1, 0)));
	}

	#[test]
	fn a_bad_signature_is_refused() {
		assert!(verify(b"not the release", "bm90IGEgc2lnbmF0dXJl").is_err());
	}
}
