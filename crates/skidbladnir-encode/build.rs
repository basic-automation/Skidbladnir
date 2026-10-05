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
//! libavif and libaom (both BSD-2) are built the same way, from the pinned submodules in
//! `third_party/`, and linked statically: libaom first, then libavif configured to use it,
//! with no libyuv and with libwebp's sharpyuv — the copy `libwebp-sys` already compiles, so
//! its headers come from the libwebp submodule at the same version (`third_party/libwebp`)
//! and its symbols from `libwebp-sys` at the final link. Then `native/avif_shim.c`, which is `avifenc`'s still-image path as a
//! function (see `src/avif.rs`), is compiled against them with libavif's own
//! `apps/shared/iccmaker.c` and `avifexif.c`. Without `CMake`, or with
//! `SKIDBLADNIR_LIBAVIF=system`, the system libavif is used instead through `pkg-config`;
//! `SKIDBLADNIR_LIBAVIF=vendored` insists on the static build.
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
//! `native/heic_shim.c`, `heif-enc`'s still-image path as a function, is compiled against
//! whichever libheif's headers.
//!
//! libjpeg-turbo (IJG and BSD-3) decodes JPEG input, because every reference tool reads JPEG
//! through it and another decoder rounds differently. It is built statically from
//! `third_party/libjpeg-turbo` like libavif, with the libjpeg 8 API the distributions ship,
//! or taken from the system with `SKIDBLADNIR_LIBJPEG=system` or without `CMake`. The
//! AVIF and HEIC shims compile `avifenc`'s and `heif-enc`'s own JPEG readers against it, and
//! `native/jpeg_shim.c` is the plain decode `cwebp` and `cjxl` do.

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
	let jpeg = Libjpeg::build();
	link_libjxl();
	link_libavif(&jpeg);
	link_libheif(&jpeg);
	let mut shim = cc::Build::new();
	shim.file("native/jpeg_shim.c").warnings(false);
	jpeg.include(&mut shim);
	shim.compile("skidjpeg");
	println!("cargo:rerun-if-changed=native/jpeg_shim.c");
	// After every shim that calls it, so a static libjpeg resolves their references.
	jpeg.link();
}

/// A macOS build for the other architecture (the Intel release is built on Apple Silicon):
/// libaom and libjpeg-turbo pick their SIMD code from `CMAKE_SYSTEM_PROCESSOR`, which is
/// the host's unless told otherwise. Returns the architecture as `CMake` spells it.
fn apple_cross(config: &mut cmake::Config) -> Option<&'static str> {
	if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
		return None;
	}
	let target = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
	let host = env::var("HOST").unwrap_or_default();
	if host.starts_with(&target) {
		return None;
	}
	let arch = if target == "aarch64" { "arm64" } else { "x86_64" };
	config.define("CMAKE_OSX_ARCHITECTURES", arch).define("CMAKE_SYSTEM_PROCESSOR", arch);
	Some(arch)
}

/// libjpeg-turbo: where its headers are, and how to link it once everything using it is
/// compiled.
struct Libjpeg {
	includes: Vec<PathBuf>,
	/// The directory of a static build, or `None` for the system library.
	static_dir: Option<PathBuf>,
	/// The system library's `pkg-config` result, linked last like the static one.
	system: Option<pkg_config::Library>,
}

impl Libjpeg {
	fn build() -> Self {
		println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBJPEG");
		let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
		let source = manifest.join("../../third_party/libjpeg-turbo");
		let choice = env::var("SKIDBLADNIR_LIBJPEG").unwrap_or_default();
		let have_cmake = Command::new("cmake").arg("--version").output().is_ok_and(|out| out.status.success());
		let system = match choice.as_str() {
			"system" => true,
			"vendored" => false,
			_ => !(have_cmake && source.join("CMakeLists.txt").exists()),
		};
		if system {
			println!("cargo:warning=linking the SYSTEM libjpeg through pkg-config (no CMake, no third_party/libjpeg-turbo, or SKIDBLADNIR_LIBJPEG=system); release builds link a static libjpeg-turbo instead");
			let library = pkg_config::Config::new().cargo_metadata(false).probe("libjpeg").unwrap_or_else(|error| panic!("JPEG input needs libjpeg: install CMake and check out third_party/libjpeg-turbo, or install the system libjpeg-turbo development files. {error}"));
			return Self { includes: library.include_paths.clone(), static_dir: None, system: Some(library) };
		}
		// The static library alone; SIMD when NASM is there, which only changes the speed.
		let built = msvc_release_build(&mut cmake::Config::new(&source)).profile("Release").define("ENABLE_SHARED", "OFF").define("ENABLE_STATIC", "ON").define("WITH_JPEG8", "ON").define("WITH_TURBOJPEG", "OFF").define("WITH_TOOLS", "OFF").define("WITH_TESTS", "OFF").define("CMAKE_POSITION_INDEPENDENT_CODE", "ON").define("CMAKE_INSTALL_LIBDIR", "lib").build();
		Self { includes: vec![built.join("include")], static_dir: Some(built.join("lib")), system: None }
	}

	fn include(&self, build: &mut cc::Build) {
		for include in &self.includes {
			build.include(include);
		}
	}

	fn link(&self) {
		if let Some(dir) = &self.static_dir {
			println!("cargo:rustc-link-search=native={}", dir.display());
			let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
			println!("cargo:rustc-link-lib=static={}", if msvc { "jpeg-static" } else { "jpeg" });
		}
		if let Some(library) = &self.system {
			for path in &library.link_paths {
				println!("cargo:rustc-link-search=native={}", path.display());
			}
			for lib in &library.libs {
				println!("cargo:rustc-link-lib={lib}");
			}
		}
	}
}

fn link_libavif(jpeg: &Libjpeg) {
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBAVIF");
	println!("cargo:rerun-if-changed=native/avif_shim.c");
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let third_party = manifest.join("../../third_party");
	let choice = env::var("SKIDBLADNIR_LIBAVIF").unwrap_or_default();
	let have_cmake = Command::new("cmake").arg("--version").output().is_ok_and(|out| out.status.success());
	let have_sources = ["libavif/CMakeLists.txt", "aom/CMakeLists.txt", "libwebp/sharpyuv/sharpyuv.h"].iter().all(|file| third_party.join(file).exists());
	let system = match choice.as_str() {
		"system" => true,
		"vendored" => false,
		_ => !(have_cmake && have_sources),
	};

	let mut shim = cc::Build::new();
	// avif_shim.c includes avifenc's JPEG reader (apps/shared/avifjpeg.c) itself; iccjpeg.c
	// is that reader's ICC helper.
	shim.file(manifest.join("native/avif_shim.c")).include(third_party.join("libavif/apps/shared")).include(third_party.join("libavif/third_party/iccjpeg")).file(third_party.join("libavif/apps/shared/iccmaker.c")).file(third_party.join("libavif/apps/shared/avifexif.c")).file(third_party.join("libavif/third_party/iccjpeg/iccjpeg.c")).warnings(false);
	jpeg.include(&mut shim);

	if system {
		println!("cargo:warning=linking the SYSTEM libavif through pkg-config (no CMake, no third_party/libavif and third_party/aom, or SKIDBLADNIR_LIBAVIF=system); release builds link a static libavif and libaom instead");
		let library = pkg_config::Config::new().atleast_version("1.4").probe("libavif").unwrap_or_else(|error| panic!("AVIF needs libavif 1.4: install CMake and check out the submodules to build it, or install the system libavif development files. {error}"));
		for include in library.include_paths {
			shim.include(include);
		}
		shim.compile("skidavif");
		return;
	}

	// libaom: static, no tools, tests, docs or examples. The decoder is built too, because
	// `avifenc -d 12,8` reconstructs the primary image to encode the residual beside it.
	let mut aom_config = cmake::Config::new(third_party.join("aom"));
	msvc_release_build(&mut aom_config);
	if let Some(arch) = apple_cross(&mut aom_config) {
		aom_config.define("AOM_TARGET_CPU", arch);
	}
	let aom = aom_config.profile("Release").define("BUILD_SHARED_LIBS", "OFF").define("CONFIG_AV1_DECODER", "1").define("CONFIG_AV1_ENCODER", "1").define("ENABLE_DOCS", "OFF").define("ENABLE_EXAMPLES", "OFF").define("ENABLE_TESTDATA", "OFF").define("ENABLE_TESTS", "OFF").define("ENABLE_TOOLS", "OFF").define("CMAKE_POLICY_VERSION_MINIMUM", "3.5").build();
	let aom_lib = ["lib", "lib64"].iter().map(|dir| aom.join(dir)).find(|dir| dir.join("libaom.a").exists() || dir.join("aom.lib").exists()).unwrap_or_else(|| aom.join("lib"));
	let aom_library = if aom_lib.join("aom.lib").exists() { aom_lib.join("aom.lib") } else { aom_lib.join("libaom.a") };

	// libavif, using that libaom and libwebp's sharpyuv. The sharpyuv "library" handed to
	// CMake is never linked by it — a static library links nothing — so it only has to
	// exist as a path; the symbols come from libwebp-sys.
	let placeholder = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join(if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") { "sharpyuv.lib" } else { "libsharpyuv.a" });
	std::fs::write(&placeholder, b"").expect("write the sharpyuv placeholder");
	let mut avif_config = cmake::Config::new(third_party.join("libavif"));
	msvc_release_build(&mut avif_config);
	apple_cross(&mut avif_config);
	let avif = avif_config
		.profile("Release")
		.define("BUILD_SHARED_LIBS", "OFF")
		.define("AVIF_CODEC_AOM", "SYSTEM")
		.define("AVIF_CODEC_AOM_DECODE", "ON")
		.define("AVIF_CODEC_AOM_ENCODE", "ON")
		.define("AOM_INCLUDE_DIR", aom.join("include"))
		.define("AOM_LIBRARY", &aom_library)
		.define("AVIF_LIBYUV", "OFF")
		.define("AVIF_LIBSHARPYUV", "SYSTEM")
		.define("LIBSHARPYUV_INCLUDE_DIR", third_party.join("libwebp"))
		.define("LIBSHARPYUV_LIBRARY", &placeholder)
		.define("AVIF_JPEG", "OFF")
		.define("AVIF_ZLIBPNG", "OFF")
		.define("AVIF_BUILD_APPS", "OFF")
		.define("AVIF_BUILD_TESTS", "OFF")
		.define("AVIF_BUILD_EXAMPLES", "OFF")
		.define("AVIF_ENABLE_WERROR", "OFF")
		// Only the library itself. libavif's `avif_static` target merges its LOCAL
		// dependencies into `libavif.a` with an `ar` script, which breaks on a build
		// directory with a space in it; with every dependency SYSTEM there is nothing to
		// merge, and `avif_internal` is the same objects.
		.build_target("avif")
		.build();
	let avif_build = avif.join("build");
	let avif_lib = [avif_build.clone(), avif_build.join("Release")].into_iter().find(|dir| dir.join("libavif_internal.a").exists() || dir.join("avif_internal.lib").exists()).unwrap_or(avif_build);

	shim.include(third_party.join("libavif/include")).compile("skidavif");
	println!("cargo:rustc-link-search=native={}", avif_lib.display());
	println!("cargo:rustc-link-search=native={}", aom_lib.display());
	println!("cargo:rustc-link-lib=static=avif_internal");
	println!("cargo:rustc-link-lib=static=aom");
	if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") && env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
		println!("cargo:rustc-link-lib=m");
	}
	println!("cargo:rustc-cfg=skidbladnir_vendored_libavif");
}

fn link_libheif(jpeg: &Libjpeg) {
	println!("cargo:rustc-check-cfg=cfg(skidbladnir_system_libheif)");
	println!("cargo:rustc-check-cfg=cfg(skidbladnir_vendored_libavif)");
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF");
	println!("cargo:rerun-if-env-changed=SKIDBLADNIR_LIBHEIF_DIR");
	println!("cargo:rerun-if-changed=native/heic_shim.c");
	let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
	let (edition, build_flag) = if env::var_os("CARGO_FEATURE_X265").is_some() { ("gpl", " --edition gpl") } else { ("standard", "") };
	let default = if edition == "gpl" { "../../build/libheif-gpl" } else { "../../build/libheif" };
	let prefix = env::var_os("SKIDBLADNIR_LIBHEIF_DIR").map_or_else(|| manifest.join(default), PathBuf::from);
	let lib = prefix.join("lib");
	let marker = prefix.join("EDITION");
	let built = ["libheif.so", "libheif.dylib", "heif.lib"].iter().any(|name| lib.join(name).exists());
	println!("cargo:rerun-if-changed={}", lib.display());
	println!("cargo:rerun-if-changed={}", marker.display());

	// `native/heic_shim.c` is `heif-enc`'s still-image path over this libheif (see src/heic.rs).
	// It is compiled first so the linker sees its references before the library.
	let mut shim = cc::Build::new();
	shim.file(manifest.join("native/heic_shim.c")).warnings(false);
	jpeg.include(&mut shim);
	// heif-enc's JPEG reader (heifio/decoder_jpeg.cc), included by this C++ file. C++20, as
	// libheif builds it: the reader uses designated initializers, which MSVC refuses before.
	let mut reader = cc::Build::new();
	reader.cpp(true).std("c++20").file(manifest.join("native/heic_jpeg.cc")).include(manifest.join("../../third_party/libheif/heifio")).include(manifest.join("../../third_party/libheif/libheif")).warnings(false);
	// On MSVC, C++ objects must agree on the C runtime, and libjxl's (from `jpegxl-src`) are
	// built against the static one — as is everything else, since `.cargo/config.toml`
	// sets `+crt-static` for Windows. It hands out and takes back its own memory
	// (`skid_heic_jpeg_release`, `_free`).
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
		reader.static_crt(true);
	}
	jpeg.include(&mut reader);
	println!("cargo:rerun-if-changed=native/heic_jpeg.cc");
	if built {
		let found = fs::read_to_string(&marker).map_or_else(|_| String::new(), |text| text.trim().to_owned());
		assert!(!found.is_empty(), "{} has a libheif but no EDITION file: it is from an older or unfinished build. Run scripts/build-libheif.sh{build_flag} again", prefix.display());
		assert!(found == edition, "{} holds the {found} edition's libheif, but this is the {edition} edition's build. Run scripts/build-libheif.sh{build_flag}, or point SKIDBLADNIR_LIBHEIF_DIR at that edition's build", prefix.display());
		shim.include(prefix.join("include"));
		reader.include(prefix.join("include"));
		shim.compile("skidheic");
		reader.compile("skidheicjpeg");
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
		let library = pkg_config::Config::new().atleast_version("1.23").cargo_metadata(false).probe("libheif").unwrap_or_else(|error| panic!("HEIC needs libheif 1.23: run scripts/build-libheif.sh{build_flag}, or install the system libheif development files. {error}"));
		for include in &library.include_paths {
			shim.include(include);
			reader.include(include);
		}
		shim.compile("skidheic");
		reader.compile("skidheicjpeg");
		pkg_config::Config::new().atleast_version("1.23").probe("libheif").expect("probed above");
		println!("cargo:rustc-cfg=skidbladnir_system_libheif");
	}
}

/// On MSVC, build a library made with `cmake` (always with `.profile("Release")`) as a
/// release build, against the C runtime Rust links.
///
/// With Visual Studio's generator, `cmake-rs` replaces `CMAKE_<LANG>_FLAGS_RELEASE` with the
/// compiler's base flags, its `/O` flags filtered out, to pass the runtime library on. That
/// drops `CMake`'s `/O2 /Ob2 /DNDEBUG`: until 1.4.1 libaom, libavif and libjpeg-turbo were
/// built unoptimised with their assertions on in every Windows build, and one of libaom's
/// fired on a superres setting `avifenc` takes (found by CI's Windows `avifenc` parity job),
/// ending the process. Defining the flags here keeps `cmake-rs` from replacing them.
///
/// With `+crt-static` (`.cargo/config.toml`) that runtime is the static one: it is named both
/// in the flags and through `CMAKE_MSVC_RUNTIME_LIBRARY` (policy `CMP0091`), so a project that
/// sets the policy either way gets it.
fn msvc_release_build(config: &mut cmake::Config) -> &mut cmake::Config {
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
		return config;
	}
	let static_crt = env::var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|features| features.split(',').any(|feature| feature == "crt-static"));
	if static_crt {
		config.static_crt(true).define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded");
	}
	let flags = format!("{} /O2 /Ob2 /DNDEBUG", if static_crt { "/MT" } else { "/MD" });
	config.define("CMAKE_C_FLAGS_RELEASE", &flags).define("CMAKE_CXX_FLAGS_RELEASE", &flags)
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
		let generator = libjxl_msvc_generator();
		if let Some(generator) = generator {
			// SAFETY: a build script's main thread, with no other thread running that reads
			// the environment; the variable is removed again once libjxl is built.
			unsafe { env::set_var("CMAKE_GENERATOR", generator) };
		}
		jpegxl_src::build();
		if generator.is_some() {
			// SAFETY: as above.
			unsafe { env::remove_var("CMAKE_GENERATOR") };
			check_libjxl_release_flags();
		}
	}
}

/// libjxl on MSVC, built by `jpegxl-src` (whose `cmake::Config` this script cannot reach),
/// has the problem `msvc_release_build` fixes for the others: with no generator named,
/// `cmake-rs` replaces `CMAKE_<LANG>_FLAGS_RELEASE`, dropping `/O2 /Ob2 /DNDEBUG`, and until
/// 1.4.2 every Windows build shipped an unoptimised libjxl with its assertions on. With a
/// generator named it leaves those flags to `CMake`, which reads `CMAKE_GENERATOR` from the
/// environment, so name the Visual Studio generator it would have chosen. The runtime is
/// still the static one: `jpegxl-src` sets `CMAKE_MSVC_RUNTIME_LIBRARY`, which libjxl's
/// policies honour.
///
/// Release builds only (`CMake`'s Release and `RelWithDebInfo`): a debug build keeps
/// `cmake-rs`'s own debug flags, since `CMake`'s add `/RTC1`, whose runtime checks C objects
/// built with `jpegxl-src`'s `/Zl` would not find a library for. `None` when not MSVC, in a
/// debug build, or when the caller chose a generator themselves.
fn libjxl_msvc_generator() -> Option<&'static str> {
	use cc::windows_registry::{VsVers, find_vs_version};
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") || env::var("OPT_LEVEL").as_deref() == Ok("0") || env::var_os("CMAKE_GENERATOR").is_some() {
		return None;
	}
	match find_vs_version() {
		Ok(VsVers::Vs18) => Some("Visual Studio 18 2026"),
		Ok(VsVers::Vs17) => Some("Visual Studio 17 2022"),
		Ok(VsVers::Vs16) => Some("Visual Studio 16 2019"),
		_ => {
			println!("cargo:warning=no Visual Studio 2019 or later found: libjxl is built with cmake-rs's flags, unoptimised");
			None
		}
	}
}

/// Fails the build if libjxl's `CMake` cache lacks the optimisation and `NDEBUG` of a
/// release build, so a Windows build cannot again ship an unoptimised libjxl unnoticed.
fn check_libjxl_release_flags() {
	let cache = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("build/CMakeCache.txt");
	let text = fs::read_to_string(&cache).unwrap_or_else(|error| panic!("read libjxl's {}: {error}", cache.display()));
	let build_type = text.lines().find_map(|line| line.strip_prefix("CMAKE_BUILD_TYPE:STRING=")).unwrap_or("Release").to_uppercase();
	for language in ["C", "CXX"] {
		let key = format!("CMAKE_{language}_FLAGS_{build_type}:STRING=");
		let flags = text.lines().find_map(|line| line.strip_prefix(key.as_str())).unwrap_or_default();
		assert!((flags.contains("/O2") || flags.contains("/O1")) && flags.contains("/DNDEBUG"), "libjxl's {key}{flags} is not an optimised release build (see libjxl_msvc_generator in build.rs)");
	}
}
