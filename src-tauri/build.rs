//! Tauri's own build step, plus where the app looks for libheif at run time.
//!
//! libheif is LGPL, so it is a shared library shipped beside the app rather than linked
//! in (see scripts/build-libheif.sh). Each installer puts it in a different place, and the
//! binary's search path names all of them:
//! - `AppImage`: `usr/lib`, beside `usr/bin` — `$ORIGIN/../lib`.
//! - `.deb`: `/usr/lib/skidbladnir`, with the binary in `/usr/bin` — `$ORIGIN/../lib/skidbladnir`.
//! - macOS: the bundle's `Frameworks` — `@executable_path/../Frameworks`.
//! - Windows: beside the `.exe`, which the loader searches first; nothing to set.
//!
//! A debug build also looks in `build/libheif` itself (`build/libheif-gpl` with the `gpl`
//! feature), so `tauri dev` runs straight from the tree. A release binary never carries
//! that absolute path.
//!
//! With the `gpl` feature, the GPL edition's config overlay (`tauri.gpl.conf.json`, and
//! `tauri.gpl.windows.conf.json` on Windows) is applied here too, under whatever the Tauri
//! CLI passes. tauri-build checks that the libheif it bundles exists, and embeds the
//! updater's endpoint: `tauri build --config tauri.gpl.conf.json` hands it the overlay,
//! but a plain `cargo build --features gpl` would otherwise look for the standard
//! edition's libheif and point the updater at the standard edition's releases.

use std::{env, fs, path::PathBuf};

use serde_json::Value;

fn main() {
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let default = if env::var_os("CARGO_FEATURE_GPL").is_some() { "../build/libheif-gpl" } else { "../build/libheif" };
	let prefix = env::var_os("SKIDBLADNIR_LIBHEIF_DIR").map_or_else(|| manifest.join(default), PathBuf::from);
	let debug = env::var("PROFILE").as_deref() == Ok("debug");
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF_DIR");

	match env::var("CARGO_CFG_TARGET_OS").as_deref() {
		Ok("linux") => {
			for path in ["$ORIGIN", "$ORIGIN/../lib", "$ORIGIN/../lib/skidbladnir"] {
				println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
			}
			if debug {
				println!("cargo:rustc-link-arg=-Wl,-rpath,{}", prefix.join("lib").display());
			}
		}
		Ok("macos") => {
			println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
			if debug {
				println!("cargo:rustc-link-arg=-Wl,-rpath,{}", prefix.join("lib").display());
			}
		}
		Ok("windows") => {
			// `tauri dev` and the tests run the binary from the target directory, so the DLL
			// goes there too. OUT_DIR is target/<profile>/build/<crate>/out.
			let dll = prefix.join("bin/heif.dll");
			if let (true, Some(target)) = (dll.exists(), PathBuf::from(env::var("OUT_DIR").unwrap_or_default()).ancestors().nth(3).map(PathBuf::from)) {
				let _ = fs::copy(&dll, target.join("heif.dll"));
				let _ = fs::copy(&dll, target.join("deps/heif.dll"));
			}
		}
		_ => {}
	}

	if env::var_os("CARGO_FEATURE_GPL").is_some() {
		apply_gpl_overlay(&manifest);
	}

	// gpui embeds a Windows application manifest of its own (Common Controls 6, per-monitor
	// DPI awareness: everything Tauri's says, and more), and an executable can carry only one.
	if env::var_os("CARGO_FEATURE_GPUI").is_some() && env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())).expect("tauri-build failed");
	} else {
		tauri_build::build();
	}
}

/// Put the GPL edition's config overlay into `TAURI_CONFIG`, beneath what is already there.
fn apply_gpl_overlay(manifest: &std::path::Path) {
	let mut overlays = vec!["tauri.gpl.conf.json"];
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		overlays.push("tauri.gpl.windows.conf.json");
	}
	let mut merged = Value::Object(serde_json::Map::new());
	for overlay in overlays {
		let path = manifest.join(overlay);
		println!("cargo:rerun-if-changed={}", path.display());
		let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("the GPL edition needs {}: {error}", path.display()));
		compose(&mut merged, serde_json::from_str(&text).unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display())));
	}
	println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
	if let Some(given) = env::var("TAURI_CONFIG").ok().filter(|given| !given.trim().is_empty()) {
		compose(&mut merged, serde_json::from_str(&given).expect("TAURI_CONFIG is JSON"));
	}
	let merged = merged.to_string();
	// tauri-build reads it below, in this process, to check what it bundles; the
	// `generate_context!` macro reads it in the compiler, to embed the config.
	// SAFETY: a build script is one thread.
	unsafe { env::set_var("TAURI_CONFIG", &merged) };
	println!("cargo:rustc-env=TAURI_CONFIG={merged}");
}

/// Fold merge patch `next` into merge patch `base`, so applying the result is applying both
/// in turn. Unlike applying a patch, a `null` is kept: it deletes from the config.
fn compose(base: &mut Value, next: Value) {
	match (base, next) {
		(Value::Object(base), Value::Object(next)) => {
			for (key, value) in next {
				match base.get_mut(&key) {
					Some(existing) if existing.is_object() && value.is_object() => compose(existing, value),
					_ => {
						base.insert(key, value);
					}
				}
			}
		}
		(base, next) => *base = next,
	}
}
