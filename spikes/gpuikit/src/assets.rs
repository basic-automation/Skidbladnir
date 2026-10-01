//! The window's own assets, compiled in: the icons the old window draws (taken from the
//! same Iconify collections it bundles), its logo, and the Fira Code cuts. Anything else is
//! gpuikit's.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! embedded {
	($($path:literal),* $(,)?) => {
		&[$(($path, include_bytes!(concat!("../assets/", $path)) as &[u8])),*]
	};
}

const FILES: &[(&str, &[u8])] = embedded![
	"skidbladnir-logo.svg",
	"fonts/FiraCode-450.ttf",
	"fonts/FiraCode-600.ttf",
	"icons/bi--filetype-heic.svg",
	"icons/codicon--debug-start.svg",
	"icons/el--inbox-box.svg",
	"icons/ic--baseline-upcoming.svg",
	"icons/iconamoon--settings-light.svg",
	"icons/iconoir--webp-format.svg",
	"icons/lucide--check.svg",
	"icons/lucide--download.svg",
	"icons/lucide--chevron-down.svg",
	"icons/lucide--folder-search.svg",
	"icons/lucide--images.svg",
	"icons/lucide--loader-circle.svg",
	"icons/lucide--x.svg",
	"icons/material-symbols--add.svg",
	"icons/material-symbols--close.svg",
	"icons/material-symbols--minimize.svg",
	"icons/mdi--maximize.svg",
	"icons/skid--jxl-format.svg",
	"icons/subway--down-2.svg",
	"icons/vscode-icons--file-type-avif.svg",
];

pub struct Assets;

impl AssetSource for Assets {
	fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
		match FILES.iter().find(|(name, _)| *name == path) {
			Some((_, bytes)) => Ok(Some(Cow::Borrowed(bytes))),
			None => gpuikit::assets().load(path),
		}
	}

	fn list(&self, path: &str) -> Result<Vec<SharedString>> {
		let mut found: Vec<SharedString> = FILES.iter().filter(|(name, _)| name.starts_with(path)).map(|(name, _)| (*name).into()).collect();
		found.extend(gpuikit::assets().list(path)?);
		Ok(found)
	}
}

/// The two Fira Code cuts, for `TextSystem::add_fonts`.
#[must_use]
pub fn fonts() -> Vec<Cow<'static, [u8]>> {
	FILES.iter().filter(|(name, _)| name.starts_with("fonts/")).map(|(_, bytes)| Cow::Borrowed(*bytes)).collect()
}
