//! In-app updates from the signed manifest `release.yml` publishes, as tauri-plugin-updater
//! does them for the Tauri window (`src-tauri/src/updater.rs`), for every way Skidbladnir is
//! installed.
//!
//! - **Check:** read this edition's manifest, take this install's platform entry (the
//!   installer-specific key first, as `linux-x86_64-appimage`, then the plain
//!   `linux-x86_64`) and offer it if its version is newer.
//! - **Install:** download the artifact, verify its minisign signature against the public key
//!   in `tauri.conf.json` and refuse it if that fails, then install it as the platform's
//!   package does:
//!   - `AppImage`: replace the file and relaunch;
//!   - .deb and .rpm: `dpkg -i` / `rpm -U` with administrator rights through `pkexec`;
//!   - Windows: run the NSIS installer passively, which replaces the app and restarts it;
//!   - macOS: unpack the `.app.tar.gz`, swap it for the running bundle (asking for
//!     administrator rights only when the bundle's folder needs them), and relaunch.

use std::{
	io::Read as _, path::{Path, PathBuf}
};

use base64::{Engine as _, engine::general_purpose::STANDARD};

/// Each edition's own manifest, so a GPL install only ever updates to a GPL build: the
/// endpoint in `tauri.conf.json` for the standard edition, and the one `tauri.gpl.conf.json`
/// overrides it with for the GPL edition. `release.yml` publishes both.
const MANIFEST: &str = if skidbladnir_encode::settings::HEIC_X265 { "https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest-gpl.json" } else { "https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest.json" };

/// `plugins.updater.pubkey` in `tauri.conf.json`: a base64-wrapped minisign public key.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwOThEOTc3MDc2NEQ2MjEKUldRaDFtUUhkOW1ZNEtMWHE0UFNrc1FtME5lWmlKdUN3cE5ta2dKaE8wOWo0UEozUFFvNmpXYk4K";

/// Which package this copy was installed from.
///
/// tauri-bundler stamps the package type into the binary it packages, and the shell reads it
/// (`tauri::utils::platform::bundle_type`) and passes it in [`crate::Options`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
	AppImage,
	Deb,
	Rpm,
	Nsis,
	Msi,
	MacApp,
}

impl Install {
	/// The suffix on the manifest's platform key.
	const fn key(self) -> &'static str {
		match self {
			Self::AppImage => "appimage",
			Self::Deb => "deb",
			Self::Rpm => "rpm",
			Self::Nsis => "nsis",
			Self::Msi => "msi",
			Self::MacApp => "app",
		}
	}
}

/// How this copy was installed: the bundler's stamp, else what the filesystem says.
#[must_use]
pub fn install_kind() -> Option<Install> {
	if let Some(stamped) = crate::options().installed_as {
		return Some(stamped);
	}
	if cfg!(target_os = "macos") {
		return mac_bundle().map(|_| Install::MacApp);
	}
	if appimage().is_some() {
		return Some(Install::AppImage);
	}
	let exe = std::env::current_exe().ok()?;
	let owned_by = |tool: &str, flag: &str| std::process::Command::new(tool).arg(flag).arg(&exe).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|status| status.success());
	if cfg!(target_os = "linux") {
		if owned_by("dpkg", "-S") {
			return Some(Install::Deb);
		}
		if owned_by("rpm", "-qf") {
			return Some(Install::Rpm);
		}
	}
	if cfg!(windows) && exe.parent().is_some_and(|folder| folder.join("uninstall.exe").is_file()) {
		return Some(Install::Nsis);
	}
	None
}

/// The running `AppImage`, when this is one: the `AppImage` runtime sets `APPIMAGE`.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
	std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|path| path.is_file())
}

/// The running macOS bundle: `<name>.app`, three levels above the executable.
#[must_use]
pub fn mac_bundle() -> Option<PathBuf> {
	let exe = std::env::current_exe().ok()?;
	let bundle = exe.parent()?.parent()?.parent()?;
	(bundle.extension().is_some_and(|extension| extension == "app")).then(|| bundle.to_path_buf())
}

/// An update on offer.
#[derive(Clone, Debug)]
pub struct Update {
	pub version: String,
	pub url: String,
	/// The `.sig` file's content, base64 as the manifest carries it.
	pub signature: String,
}

/// The manifest's platform keys for this install, most specific first.
fn platform_keys() -> Vec<String> {
	let os = match std::env::consts::OS {
		"macos" => "darwin",
		other => other,
	};
	let base = format!("{os}-{}", std::env::consts::ARCH);
	install_kind().map(|install| format!("{base}-{}", install.key())).into_iter().chain([base]).collect()
}

/// The update on offer, if the manifest has one newer than `current` for this install.
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

/// What to do once an update is installed.
#[derive(Debug)]
pub enum Restart {
	/// Start this program, then quit.
	Launch(PathBuf),
	/// The installer restarts the app itself (Windows); quit now.
	Quit,
}

/// Download, verify and install `update`.
///
/// # Errors
///
/// Why it could not, as text for the banner.
pub fn install(update: &Update, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<Restart, String> {
	let Some(kind) = install_kind() else {
		return Err("this copy was not installed from a release package, so it cannot update itself; install the latest release instead".to_owned());
	};
	let data = download(update, progress)?;
	verify(&data, &update.signature)?;
	match kind {
		Install::AppImage => {
			let target = appimage().ok_or("the AppImage this copy runs from could not be found")?;
			replace(&target, &data)?;
			Ok(Restart::Launch(target))
		}
		Install::Deb => install_package(&data, "deb", "dpkg", "-i").map(|()| Restart::Launch(std::env::current_exe().unwrap_or_default())),
		Install::Rpm => install_package(&data, "rpm", "rpm", "-U").map(|()| Restart::Launch(std::env::current_exe().unwrap_or_default())),
		Install::Nsis | Install::Msi => windows::run_installer(&data, kind).map(|()| Restart::Quit),
		Install::MacApp => mac::replace_bundle(&data).map(Restart::Launch),
	}
}

/// Replace the `AppImage` at `target` with `data`, which has been verified. Written beside it
/// and renamed over it, so a failure part-way leaves the old one in place.
///
/// # Errors
///
/// The filesystem's failure, as text.
pub fn replace(target: &Path, data: &[u8]) -> Result<(), String> {
	let staged = target.with_extension("AppImage.update");
	std::fs::write(&staged, data).map_err(|error| format!("could not write {}: {error}", staged.display()))?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt as _;
		std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(|error| format!("could not make {} executable: {error}", staged.display()))?;
	}
	std::fs::rename(&staged, target).map_err(|error| {
		let _ = std::fs::remove_file(&staged);
		format!("could not replace {}: {error}", target.display())
	})
}

/// Install a .deb or .rpm with administrator rights, through `pkexec` as
/// tauri-plugin-updater does.
fn install_package(data: &[u8], extension: &str, tool: &str, flag: &str) -> Result<(), String> {
	let folder = std::env::temp_dir().join(format!("skidbladnir-update-{}", std::process::id()));
	std::fs::create_dir_all(&folder).map_err(|error| format!("could not make a folder for the update: {error}"))?;
	let package = folder.join(format!("package.{extension}"));
	std::fs::write(&package, data).map_err(|error| format!("could not write the update: {error}"))?;
	let status = std::process::Command::new("pkexec").arg(tool).arg(flag).arg(&package).status();
	let _ = std::fs::remove_dir_all(&folder);
	match status {
		Ok(status) if status.success() => Ok(()),
		Ok(status) if status.code() == Some(126) => Err("installing needs administrator rights, which were not given".to_owned()),
		Ok(status) => Err(format!("{tool} could not install the update ({status})")),
		Err(error) => Err(format!("could not ask for administrator rights through pkexec: {error}")),
	}
}

#[cfg(windows)]
mod windows {
	use std::{ffi::OsStr, os::windows::ffi::OsStrExt as _};

	use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOW};

	use super::Install;

	/// Run the installer as tauri-plugin-updater does for `installMode: "passive"`: `/P`
	/// (passive), `/UPDATE`, and `/R` so it restarts the app when done. Through
	/// `ShellExecuteW`, so Windows can ask for elevation if the installer needs it.
	pub fn run_installer(data: &[u8], kind: Install) -> Result<(), String> {
		let folder = std::env::temp_dir().join(format!("skidbladnir-update-{}", std::process::id()));
		std::fs::create_dir_all(&folder).map_err(|error| format!("could not make a folder for the update: {error}"))?;
		let (file, parameters) = if kind == Install::Msi {
			let path = folder.join("Skidbladnir.msi");
			std::fs::write(&path, data).map_err(|error| format!("could not write the update: {error}"))?;
			let system = std::env::var("SYSTEMROOT").map_or_else(|_| "msiexec.exe".to_owned(), |root| format!("{root}\\System32\\msiexec.exe"));
			(system, format!("/i \"{}\" /passive /promptrestart", path.display()))
		} else {
			let path = folder.join("Skidbladnir-setup.exe");
			std::fs::write(&path, data).map_err(|error| format!("could not write the update: {error}"))?;
			(path.display().to_string(), "/P /UPDATE /R".to_owned())
		};
		let wide = |text: &str| OsStr::new(text).encode_wide().chain([0]).collect::<Vec<u16>>();
		let (operation, file, parameters) = (wide("open"), wide(&file), wide(&parameters));
		// SAFETY: every pointer is to a NUL-terminated wide string that outlives the call.
		let result = unsafe { ShellExecuteW(std::ptr::null_mut(), operation.as_ptr(), file.as_ptr(), parameters.as_ptr(), std::ptr::null(), SW_SHOW) };
		if result as isize <= 32 { Err(format!("could not start the installer: {}", std::io::Error::last_os_error())) } else { Ok(()) }
	}
}

#[cfg(not(windows))]
mod windows {
	use super::Install;

	pub fn run_installer(_: &[u8], _: Install) -> Result<(), String> {
		Err("a Windows installer can only be run on Windows".to_owned())
	}
}

#[cfg(target_os = "macos")]
mod mac {
	use std::path::PathBuf;

	/// Unpack the `.app.tar.gz` beside nothing it could clash with, move the running bundle
	/// to a backup, put the new one in its place, and drop the backup. If the bundle's folder
	/// is not writable (/Applications for a standard user), the moves are made by
	/// `osascript` with administrator privileges, as tauri-plugin-updater does.
	pub fn replace_bundle(data: &[u8]) -> Result<PathBuf, String> {
		let bundle = super::mac_bundle().ok_or("the app bundle this copy runs from could not be found")?;
		let work = std::env::temp_dir().join(format!("skidbladnir-update-{}", std::process::id()));
		let unpacked = work.join("new");
		std::fs::create_dir_all(&unpacked).map_err(|error| format!("could not make a folder for the update: {error}"))?;
		tar::Archive::new(flate2::read::GzDecoder::new(data)).unpack(&unpacked).map_err(|error| format!("could not unpack the update: {error}"))?;
		let new_bundle = std::fs::read_dir(&unpacked).map_err(|error| error.to_string())?.filter_map(Result::ok).map(|entry| entry.path()).find(|path| path.extension().is_some_and(|extension| extension == "app")).ok_or("the update holds no app bundle")?;
		let backup = work.join("previous.app");
		if std::fs::rename(&bundle, &backup).is_ok() {
			if let Err(error) = std::fs::rename(&new_bundle, &bundle) {
				let _ = std::fs::rename(&backup, &bundle);
				return Err(format!("could not put the new app in place: {error}"));
			}
		} else {
			let script = format!("do shell script \"rm -rf '{0}' && mv -f '{1}' '{0}'\" with administrator privileges", bundle.display(), new_bundle.display());
			let status = std::process::Command::new("osascript").arg("-e").arg(script).status().map_err(|error| error.to_string())?;
			if !status.success() {
				return Err("installing needs administrator rights, which were not given".to_owned());
			}
		}
		let _ = std::fs::remove_dir_all(&work);
		Ok(bundle)
	}
}

#[cfg(not(target_os = "macos"))]
mod mac {
	pub fn replace_bundle(_: &[u8]) -> Result<std::path::PathBuf, String> {
		Err("an app bundle can only be installed on macOS".to_owned())
	}
}

/// Start what was installed, as `app.restart()` does: on macOS by opening the bundle.
///
/// # Errors
///
/// The launch's failure, as text.
pub fn relaunch(target: &Path) -> Result<(), String> {
	let mut command = if cfg!(target_os = "macos") {
		let mut open = std::process::Command::new("open");
		open.arg("-n").arg(target);
		open
	} else {
		std::process::Command::new(target)
	};
	command.spawn().map(|_| ()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
	use super::{Install, parse, verify};

	#[test]
	fn versions_compare_numerically() {
		assert!(parse("1.10.0") > parse("1.9.9"));
		assert_eq!(parse("v1.1.0".trim_start_matches('v')), Some((1, 1, 0)));
	}

	#[test]
	fn a_bad_signature_is_refused() {
		assert!(verify(b"not the release", "bm90IGEgc2lnbmF0dXJl").is_err());
	}

	#[test]
	fn platform_keys_match_the_manifest() {
		assert_eq!([Install::AppImage, Install::Deb, Install::Rpm, Install::Nsis, Install::MacApp].map(Install::key), ["appimage", "deb", "rpm", "nsis", "app"]);
	}
}
