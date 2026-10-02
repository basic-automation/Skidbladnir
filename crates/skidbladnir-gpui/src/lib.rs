//! Skidbladnir's window, drawn with gpui (and gpuikit's text fields): the default UI since
//! 1.3.0. It matches the Nuxt window it replaces control for control — see README.md for how
//! that was checked, and the few things that differ.
//!
//! The Tauri shell (`src-tauri`) calls [`run`]. Encoding is the same `skidbladnir-encode`
//! calls the webview window makes through its commands, and the preferences, presets and
//! preview are the shell's own code, compiled in (`src/shell`).

mod about;
mod app;
mod assets;
mod color_picker;
mod controls;
mod css_text;
mod dialogs;
mod fade;
mod panels;
mod shell;
mod theme;
pub mod updater;

use std::sync::OnceLock;

use gpui::{App, AppContext, Application, Bounds, KeyBinding, WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions, actions, px, size};
use skidbladnir_encode::{
	encoder::{linked_decoder_version, linked_encoder_version}, settings::HEIC_X265
};

actions!(skidbladnir, [FocusNext, FocusPrevious]);

/// The line the settings popover shows: `commands::encoder_version`, unchanged.
#[must_use]
pub fn backend_version() -> String {
	let (emajor, eminor, erevision) = linked_encoder_version();
	let (dmajor, dminor, drevision) = linked_decoder_version();
	let (jmajor, jminor, jpatch) = skidbladnir_encode::jxl::linked_version();
	let label = if HEIC_X265 { " (GPL edition)" } else { "" };
	format!("Skidbladnir {}{label} · libwebp encoder {emajor}.{eminor}.{erevision} · decoder {dmajor}.{dminor}.{drevision} · libavif {} ({}) · libjxl {jmajor}.{jminor}.{jpatch} · libheif {} with {}", app::app_version(), skidbladnir_encode::avif::linked_version(), skidbladnir_encode::avif::linked_codecs(), skidbladnir_encode::heic::linked_version(), skidbladnir_encode::heic::encoder_name())
}

/// What the shell tells the window about itself.
#[derive(Clone, Copy, Debug)]
pub struct Options {
	/// The product version: `src-tauri/Cargo.toml`'s, the only copy.
	pub version: &'static str,
	/// The package tauri-bundler stamped into the binary, when it packaged it.
	pub installed_as: Option<updater::Install>,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

/// The options [`run`] was given.
pub(crate) fn options() -> Options {
	*OPTIONS.get_or_init(|| Options { version: "0.0.0", installed_as: None })
}

/// Open the window and run until it closes.
///
/// If the window cannot be opened (no GPU the renderer can use, say), this copy starts
/// itself again with `SKIDBLADNIR_UI=webview`, which the shell takes as asking for the
/// webview window, and quits.
pub fn run(given: Options) {
	let _ = OPTIONS.set(given);
	Application::with_platform(gpui_platform::current_platform(false)).with_assets(assets::Assets).run(|cx: &mut App| {
		gpuikit::init(cx);
		if let Err(error) = cx.text_system().add_fonts(assets::fonts()) {
			eprintln!("Skidbladnir: the bundled fonts did not load: {error}");
		}
		cx.bind_keys([KeyBinding::new("tab", FocusNext, None), KeyBinding::new("shift-tab", FocusPrevious, None)]);
		// tauri.conf.json: 1280×960, at least 720×560, centred, frameless and transparent.
		let bounds = Bounds::centered(None, size(px(1280.), px(960.)), cx);
		let opened = cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: None, window_min_size: Some(size(px(720.), px(560.))), window_background: if std::env::var_os("SKID_OPAQUE").is_some() { WindowBackgroundAppearance::Opaque } else { WindowBackgroundAppearance::Transparent }, window_decorations: Some(WindowDecorations::Client), app_id: Some("skidbladnir".to_owned()), ..Default::default() }, |window, cx| {
			window.set_window_title("Skidbladnir");
			cx.new(|cx| app::Skid::new(window, cx))
		});
		match opened {
			Ok(_) => cx.activate(true),
			Err(error) => {
				eprintln!("Skidbladnir: the window could not be opened ({error:#}); starting the webview window instead");
				if let Ok(exe) = std::env::current_exe() {
					let _ = std::process::Command::new(exe).args(std::env::args_os().skip(1)).env("SKIDBLADNIR_UI", "webview").spawn();
				}
				cx.quit();
			}
		}
	});
}
