//! Link libjxl, the JPEG XL reference encoder.
//!
//! By default libjxl is built from source and linked statically, the same way libwebp is:
//! the app then carries its own encoder on every platform and the output does not depend
//! on whatever `libjxl` a user's system happens to have. That build needs `CMake` and a C++
//! compiler, which every CI runner has.
//!
//! A dev host without `CMake` can still build: if `cmake` is not on `PATH`, or
//! `SKIDBLADNIR_LIBJXL=system` is set, the system libjxl is linked dynamically through
//! `pkg-config` instead, with a warning. `SKIDBLADNIR_LIBJXL=vendored` forces the static
//! build and fails loudly if it cannot run, which is what CI and releases want.
//!
//! libheif is different: it is LGPL, so it is linked **dynamically** and shipped beside the
//! app. `scripts/build-libheif.sh` builds it, with the decoder and one HEVC encoder inside,
//! into `build/libheif` for the standard edition (Kvazaar) or `build/libheif-gpl` for the
//! GPL edition (x265, the `x265` feature), or wherever `SKIDBLADNIR_LIBHEIF_DIR` points.
//! The build records its edition in an `EDITION` file beside the library, and this refuses
//! a library of the other edition: a standard build that linked the GPL libheif would ship
//! x265 in an app that says it is ISC.
//!
//! Without that build the system libheif is linked instead, through `pkg-config`, with a
//! warning, and `cfg(skidbladnir_system_libheif)` lets the code fall back to whatever HEVC
//! encoder the system library has. `SKIDBLADNIR_LIBHEIF=vendored` refuses that fallback.

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
	link_libjxl();
	link_libheif();
}

fn link_libheif() {
	println!("cargo:rustc-check-cfg=cfg(skidbladnir_system_libheif)");
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF");
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF_DIR");
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let (edition, build_flag) = if env::var_os("CARGO_FEATURE_X265").is_some() { ("gpl", " --edition gpl") } else { ("standard", "") };
	let default = if edition == "gpl" { "../../build/libheif-gpl" } else { "../../build/libheif" };
	let prefix = env::var_os("SKIDBLADNIR_LIBHEIF_DIR").map_or_else(|| manifest.join(default), PathBuf::from);
	let lib = prefix.join("lib");
	let marker = prefix.join("EDITION");
	let built = ["libheif.so", "libheif.dylib", "heif.lib"].iter().any(|name| lib.join(name).exists());
	println!("cargo:rerun-if-changed={}", lib.display());
	println!("cargo:rerun-if-changed={}", marker.display());

	if built {
		let found = fs::read_to_string(&marker).map_or_else(|_| String::new(), |text| text.trim().to_owned());
		assert!(!found.is_empty(), "{} has a libheif but no EDITION file: it is from an older or unfinished build. Run scripts/build-libheif.sh{build_flag} again", prefix.display());
		assert!(found == edition, "{} holds the {found} edition's libheif, but this is the {edition} edition's build. Run scripts/build-libheif.sh{build_flag}, or point SKIDBLADNIR_LIBHEIF_DIR at that edition's build", prefix.display());
		println!("cargo:rustc-link-search=native={}", lib.display());
		println!("cargo:rustc-link-lib=dylib=heif");
		// This package's own tests find the library where it was built. The app binary sets
		// its own search path in src-tauri/build.rs, relative to where it is installed.
		if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
			println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
		}
	} else if env::var("SKIDBLADNIR_LIBHEIF").as_deref() == Ok("vendored") {
		panic!("SKIDBLADNIR_LIBHEIF=vendored, but there is no libheif in {}: run scripts/build-libheif.sh{build_flag} first", lib.display());
	} else {
		println!("cargo:warning=linking the SYSTEM libheif through pkg-config (no {}); release builds ship their own, with the {edition} edition's encoder", prefix.display());
		pkg_config::Config::new().atleast_version("1.20").probe("libheif").unwrap_or_else(|error| panic!("HEIC needs libheif: run scripts/build-libheif.sh{build_flag}, or install the system libheif development files. {error}"));
		println!("cargo:rustc-cfg=skidbladnir_system_libheif");
	}
}

fn link_libjxl() {
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBJXL");
	let choice = env::var("SKIDBLADNIR_LIBJXL").unwrap_or_default();
	let have_cmake = Command::new("cmake").arg("--version").output().is_ok_and(|out| out.status.success());

	let system = match choice.as_str() {
		"system" => true,
		"vendored" => false,
		_ => !have_cmake,
	};

	if system {
		println!("cargo:warning=linking the SYSTEM libjxl through pkg-config (no CMake, or SKIDBLADNIR_LIBJXL=system); release builds link a static libjxl instead");
		for library in ["libjxl", "libjxl_threads"] {
			pkg_config::Config::new().atleast_version("0.11").probe(library).unwrap_or_else(|error| panic!("JPEG XL needs libjxl: install CMake to build it from source, or the system libjxl development files. {error}"));
		}
	} else {
		jpegxl_src::build();
	}
}
