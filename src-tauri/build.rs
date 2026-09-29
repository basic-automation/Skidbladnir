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

use std::{env, fs, path::PathBuf};

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

	tauri_build::build();
}
