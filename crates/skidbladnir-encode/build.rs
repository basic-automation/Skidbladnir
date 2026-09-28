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

use std::{env, process::Command};

fn main() {
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
