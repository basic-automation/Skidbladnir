//! The licence texts the installers carry, for the window's About view.
//!
//! Every installer bundles the app's `LICENSE` and the third-party notices, and the GPL
//! edition the GPL as `COPYING` (`bundle.resources` in `tauri.conf.json` and
//! `tauri.gpl.conf.json`). The window asks for one of them by a fixed name, never by a
//! path, so nothing it sends can make this read any other file.

use std::{
	fs, io::Read as _, path::{Path, PathBuf}
};

use serde::Deserialize;

/// The largest document read: the notices are about 750 KB, and anything far beyond that
/// is not one of ours.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// One of the bundled documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LegalDocument {
	/// Skidbladnir's own licence (ISC).
	License,
	/// The licences and notices of the libraries the build carries.
	Notices,
	/// The GNU GPL, version 3: the GPL edition's licence.
	Copying,
}

impl LegalDocument {
	/// The name the document is bundled under, beside the app's other resources.
	#[must_use]
	pub const fn file_name(self) -> &'static str {
		match self {
			Self::License => "LICENSE",
			Self::Notices => "THIRD-PARTY-NOTICES.md",
			Self::Copying => "COPYING",
		}
	}
}

/// Read `document` from the resource directory.
///
/// # Errors
///
/// A message for display if it is missing (the standard edition has no `COPYING`),
/// unreadable, too large or not text.
pub fn read(resource_directory: &Path, document: LegalDocument) -> Result<String, String> {
	let path: PathBuf = resource_directory.join(document.file_name());
	let file = fs::File::open(&path).map_err(|error| format!("`{}` is not in this installation: {error}", document.file_name()))?;
	let mut bytes = Vec::new();
	file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|error| format!("could not read `{}`: {error}", document.file_name()))?;
	if bytes.len() as u64 > MAX_BYTES {
		return Err(format!("`{}` is larger than any document Skidbladnir ships", document.file_name()));
	}
	String::from_utf8(bytes).map_err(|_| format!("`{}` is not text", document.file_name()))
}

#[cfg(test)]
mod tests {
	use std::fs;

	use super::{LegalDocument, MAX_BYTES, read};

	/// The names asked for are the names the bundle configuration installs, for both
	/// editions, so the About view finds them in a real installation.
	#[test]
	fn every_document_is_one_the_installers_bundle() {
		let standard: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).expect("parse tauri.conf.json");
		let gpl: serde_json::Value = serde_json::from_str(include_str!("../tauri.gpl.conf.json")).expect("parse tauri.gpl.conf.json");
		let installed = |config: &serde_json::Value| -> Vec<String> { config["bundle"]["resources"].as_object().map_or_else(Vec::new, |map| map.values().filter_map(|target| target.as_str().map(str::to_owned)).collect()) };
		let (standard, gpl) = (installed(&standard), installed(&gpl));
		for document in [LegalDocument::License, LegalDocument::Notices] {
			assert!(standard.iter().any(|name| name == document.file_name()), "the standard edition bundles {}", document.file_name());
		}
		// The GPL configuration is merged over the standard one: it swaps the notices and adds COPYING.
		assert!(gpl.iter().any(|name| name == LegalDocument::Notices.file_name()));
		assert!(gpl.iter().any(|name| name == LegalDocument::Copying.file_name()));
	}

	#[test]
	fn reads_a_document_and_refuses_what_is_not_one() {
		let dir = std::env::temp_dir().join(format!("skidbladnir-legal-{}", std::process::id()));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).expect("create the scratch directory");
		fs::write(dir.join("LICENSE"), "ISC License\n").expect("write");
		assert_eq!(read(&dir, LegalDocument::License), Ok("ISC License\n".to_owned()));
		assert!(read(&dir, LegalDocument::Copying).expect_err("missing").contains("not in this installation"));
		fs::write(dir.join("THIRD-PARTY-NOTICES.md"), [0xff_u8, 0xfe, 0x00]).expect("write");
		assert!(read(&dir, LegalDocument::Notices).expect_err("binary").contains("not text"));
		fs::write(dir.join("COPYING"), vec![b'x'; usize::try_from(MAX_BYTES).expect("fits") + 1]).expect("write");
		assert!(read(&dir, LegalDocument::Copying).expect_err("too large").contains("larger"));
		let _ = fs::remove_dir_all(&dir);
	}

	/// The window sends the document's name in camelCase.
	#[test]
	fn the_window_names_documents_in_camel_case() {
		assert_eq!(serde_json::from_str::<LegalDocument>(r#""notices""#).expect("parse"), LegalDocument::Notices);
		assert!(serde_json::from_str::<LegalDocument>(r#""../etc/passwd""#).is_err());
	}
}
