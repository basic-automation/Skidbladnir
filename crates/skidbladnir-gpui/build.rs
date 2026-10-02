//! Where this package's own tests and examples find libheif: where it was built, as the
//! encode crate's tests do (its search path does not reach this package's binaries). The
//! app binary sets its own, relative to where it is installed, in src-tauri/build.rs.

use std::{env, path::PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF_DIR");
	if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() != Ok("unix") {
		return;
	}
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let default = if env::var_os("CARGO_FEATURE_GPL").is_some() { "../../build/libheif-gpl" } else { "../../build/libheif" };
	let prefix = env::var_os("SKIDBLADNIR_LIBHEIF_DIR").map_or_else(|| manifest.join(default), PathBuf::from);
	println!("cargo:rustc-link-arg=-Wl,-rpath,{}", prefix.join("lib").display());
}
