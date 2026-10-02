//! The `AppStream` metadata the `.deb` and the `AppImage` install
//! (`resources/linux/*.metainfo.xml`) must describe this build: the app's identifier, the
//! desktop entry Tauri writes, each edition's licence, and — the part that would quietly go
//! stale — the newest release must be the version being built.

const STANDARD: &str = include_str!("../../resources/linux/com.basicautomation.skidbladnir.metainfo.xml");
const GPL: &str = include_str!("../../resources/linux/com.basicautomation.skidbladnir.gpl.metainfo.xml");

/// The text of the first `<tag>…</tag>` or the value of the first `tag="…"` attribute.
fn element<'a>(xml: &'a str, tag: &str) -> &'a str {
	let open = format!("<{tag}>");
	let start = xml.find(&open).unwrap_or_else(|| panic!("no <{tag}>")) + open.len();
	let end = xml[start..].find('<').expect("a closing tag") + start;
	&xml[start..end]
}

fn attribute<'a>(xml: &'a str, after: &str, name: &str) -> &'a str {
	let from = xml.find(after).unwrap_or_else(|| panic!("no {after}"));
	let key = format!("{name}=\"");
	let start = xml[from..].find(&key).expect("the attribute") + from + key.len();
	let end = xml[start..].find('"').expect("a closing quote") + start;
	&xml[start..end]
}

#[test]
fn the_metainfo_describes_this_build() {
	let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).expect("parse tauri.conf.json");
	let identifier = config["identifier"].as_str().expect("an identifier");
	let desktop = format!("{}.desktop", config["productName"].as_str().expect("a product name"));
	for (xml, licence) in [(STANDARD, "ISC"), (GPL, "GPL-3.0-or-later")] {
		assert_eq!(element(xml, "id"), identifier);
		assert_eq!(element(xml, "launchable type=\"desktop-id\""), desktop, "Tauri names the desktop entry after the product");
		assert_eq!(element(xml, "project_license"), licence);
		assert_eq!(attribute(xml, "<release ", "version"), env!("CARGO_PKG_VERSION"), "add a <release> for this version, newest first");
	}
}

/// Both editions install their file at the one path software centres read.
#[test]
fn each_edition_installs_its_own_metainfo() {
	let path = "/usr/share/metainfo/com.basicautomation.skidbladnir.metainfo.xml";
	let linux: serde_json::Value = serde_json::from_str(include_str!("../tauri.linux.conf.json")).expect("parse");
	let gpl: serde_json::Value = serde_json::from_str(include_str!("../tauri.gpl.conf.json")).expect("parse");
	for bundle in ["deb", "rpm", "appimage"] {
		assert_eq!(linux["bundle"]["linux"][bundle]["files"][path], "../resources/linux/com.basicautomation.skidbladnir.metainfo.xml", "{bundle}");
		assert_eq!(gpl["bundle"]["linux"][bundle]["files"][path], "../resources/linux/com.basicautomation.skidbladnir.gpl.metainfo.xml", "{bundle}");
	}
}
