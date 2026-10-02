//! Try the updater end to end against the real manifest, into a throwaway file:
//!
//! `APPIMAGE=/tmp/x.AppImage cargo run --release --example update_install -- 1.0.0`
//!
//! Checks for an update newer than the given version, downloads it, verifies its signature
//! with the app's key, installs it over `$APPIMAGE`, and confirms a one-byte change is refused.

use skidbladnir_gpui::updater;

fn main() {
	let current = std::env::args().nth(1).unwrap_or_else(|| "1.0.0".to_owned());
	let update = updater::check(&current).expect("an update newer than that is on offer");
	println!("offered {} at {}", update.version, update.url);
	let target = updater::appimage().expect("APPIMAGE names an existing file");
	println!("install kind {:?}", updater::install_kind());
	let mut reported = 0;
	let installed = updater::install(&update, &mut |done, total| {
		if let Some(total) = total
			&& done * 4 / total > reported
		{
			reported = done * 4 / total;
			println!("downloaded {}%", reported * 25);
		}
	})
	.expect("the update installs");
	let updater::Restart::Launch(installed) = installed else { panic!("an AppImage relaunches itself") };
	let data = std::fs::read(&installed).expect("the installed file reads");
	println!("installed {} bytes into {}, signature verified", data.len(), target.display());
	let mut tampered = data;
	tampered[1000] ^= 1;
	assert!(updater::verify(&tampered, &update.signature).is_err(), "a changed byte must be refused");
	println!("a one-byte change is refused");
}
