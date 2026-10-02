//! Skidbladnir's window rebuilt with gpui and gpuikit, to match the 1.0.0 Tauri window
//! exactly, so the two can be compared fairly. See README.md.

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
mod updater;

use gpui::{App, AppContext, Application, Bounds, KeyBinding, WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions, actions, px, size};
use skidbladnir_encode::{encoder::{linked_decoder_version, linked_encoder_version}, settings::HEIC_X265};

actions!(skidbladnir, [FocusNext, FocusPrevious]);

/// The line the settings popover shows: `commands::encoder_version`, unchanged.
#[must_use]
pub fn backend_version() -> String {
	let (emajor, eminor, erevision) = linked_encoder_version();
	let (dmajor, dminor, drevision) = linked_decoder_version();
	let (jmajor, jminor, jpatch) = skidbladnir_encode::jxl::linked_version();
	let label = if HEIC_X265 { " (GPL edition)" } else { "" };
	format!(
		"Skidbladnir {}{label} · libwebp encoder {emajor}.{eminor}.{erevision} · decoder {dmajor}.{dminor}.{drevision} · libavif {} ({}) · libjxl {jmajor}.{jminor}.{jpatch} · libheif {} with {}",
		app::APP_VERSION,
		skidbladnir_encode::avif::linked_version(),
		skidbladnir_encode::avif::linked_codecs(),
		skidbladnir_encode::heic::linked_version(),
		skidbladnir_encode::heic::encoder_name()
	)
}

fn main() {
	Application::with_platform(gpui_platform::current_platform(false)).with_assets(assets::Assets).run(|cx: &mut App| {
		gpuikit::init(cx);
		cx.text_system().add_fonts(assets::fonts()).expect("the bundled fonts load");
		cx.bind_keys([KeyBinding::new("tab", FocusNext, None), KeyBinding::new("shift-tab", FocusPrevious, None)]);
		// tauri.conf.json: 1280×960, at least 720×560, centred, frameless and transparent.
		let bounds = Bounds::centered(None, size(px(1280.), px(960.)), cx);
		cx.open_window(
			WindowOptions {
				window_bounds: Some(WindowBounds::Windowed(bounds)),
				titlebar: None,
				window_min_size: Some(size(px(720.), px(560.))),
				window_background: WindowBackgroundAppearance::Transparent,
				window_decorations: Some(WindowDecorations::Client),
				app_id: Some("skidbladnir".to_owned()),
				..Default::default()
			},
			|window, cx| cx.new(|cx| app::Skid::new(window, cx)),
		)
		.expect("could not open the window");
		cx.activate(true);
	});
}
