//! The check half of `src-tauri/src/updater.rs`: read the signed manifest `release.yml`
//! publishes and say whether it offers a newer version for this platform. Installing — the
//! download, the minisign check and the swap — is tauri-plugin-updater's, and has no
//! counterpart here (see README.md).

/// The endpoint in `tauri.conf.json`.
const MANIFEST: &str = "https://raw.githubusercontent.com/basic-automation/Skidbladnir/updater/latest.json";

/// The version on offer, if it is newer than `current` and has a build for this platform.
#[must_use]
pub fn check(current: &str) -> Option<String> {
	let body = ureq::get(MANIFEST).call().ok()?.body_mut().read_to_string().ok()?;
	let manifest: serde_json::Value = serde_json::from_str(&body).ok()?;
	let version = manifest.get("version")?.as_str()?.trim_start_matches('v').to_owned();
	let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
	manifest.get("platforms")?.get(&platform)?;
	(parse(&version)? > parse(current)?).then_some(version)
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
	let mut parts = version.split(['.', '-', '+']).map(str::parse::<u64>);
	Some((parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?))
}
