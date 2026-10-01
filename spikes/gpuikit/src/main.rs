//! The gpuikit spike: Skidbladnir's WebP options and before/after preview, with no webview.
//!
//! What it exercises, because these are the parts of the real window a migration would
//! have to get right:
//!
//! - **The encoder controls as gpuikit components**: sliders, switches, a select and a
//!   toggle group, each bound straight to a field of [`WebpSettings`]. There is no IPC
//!   and no TypeScript mirror of the settings: the panel edits the struct the encoder
//!   reads.
//! - **The native file dialogs gpui has built in** (`prompt_for_paths`,
//!   `prompt_for_new_path`) in place of `tauri-plugin-dialog`.
//! - **Encoding off the UI thread**, re-run on every change, with a stale result dropped
//!   rather than shown.
//! - **The preview as GPU textures**: both sides go from RGBA straight into a
//!   [`RenderImage`], where the Tauri app round-trips each one through a lossless WebP
//!   and a base64 `data:` URL.
//!
//! Out of scope: AVIF, JPEG XL, HEIC, geometry, presets, batches, the updater, the Paleday
//! theme and the frameless window. See README.md for what the spike is meant to answer.

use std::{
	path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}
};

use gpui::{
	App, AppContext, Application, Bounds, Context, Entity, InteractiveElement, IntoElement, ObjectFit, ParentElement, PathPromptOptions, Render, RenderImage, SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription, Task, TitlebarOptions, Window, WindowBounds, WindowOptions, div, img, prelude::FluentBuilder, px, size
};
use gpuikit::{
	elements::{
		button::button, select::{SelectState, select}, slider::{Slider, SliderChanged, slider}, switch::{Switch, SwitchChanged, switch}, toggle_group::{ToggleGroup, ToggleGroupChanged, toggle_group, toggle_option}
	}, layout::{h_stack, v_stack}, theme::{ActiveTheme, Themeable}
};
use skidbladnir_encode::{
	AlphaFiltering, EncodeJob, FilterType, ImageHint, SourceImage, WebpSettings, encode_source_with_progress, source
};

/// How long the settings must sit still before an encode starts. A slider drag emits a
/// change per frame; without this every one of them would start an encode.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// One finished preview encode.
struct Encoded {
	/// The encoded file's size: what would be written.
	bytes: usize,
	/// The encoded file decoded back to pixels by libwebp, ready to draw.
	image: Arc<RenderImage>,
	/// Wall time for encode plus decode, to show the cost of a setting.
	took: Duration,
}

/// The source the user picked, decoded once and kept for every re-encode.
struct Loaded {
	path: PathBuf,
	image: Arc<SourceImage>,
	/// The file's size on disk.
	bytes: u64,
	/// The source pixels as a texture, for the left-hand side.
	texture: Arc<RenderImage>,
}

struct Spike {
	job: EncodeJob,
	loaded: Option<Loaded>,
	encoded: Option<Encoded>,
	/// The last failure, shown in the status line until the next success.
	error: Option<SharedString>,
	/// Bumped on every settings change. A result is applied only if it was started for the
	/// current generation, so a slow encode never overwrites a newer one.
	generation: u64,
	encoding: bool,
	/// Replacing this drops (and so cancels) the previous pending encode.
	encode_task: Option<Task<()>>,
	controls: Controls,
	_subscriptions: Vec<Subscription>,
}

/// The control entities. gpuikit's stateful components are entities the parent owns and
/// subscribes to, rather than props bound to the parent's state the way Vue's are.
struct Controls {
	lossless: Entity<Switch>,
	quality: Entity<Slider>,
	method: Entity<Slider>,
	alpha_quality: Entity<Slider>,
	sns: Entity<Slider>,
	filter_strength: Entity<Slider>,
	filter_sharpness: Entity<Slider>,
	filter_type: Entity<ToggleGroup<FilterType>>,
	autofilter: Entity<Switch>,
	segments: Entity<Slider>,
	passes: Entity<Slider>,
	near_lossless: Entity<Slider>,
	image_hint: Entity<SelectState<ImageHint>>,
	alpha_filtering: Entity<SelectState<AlphaFiltering>>,
	sharp_yuv: Entity<Switch>,
	exact: Entity<Switch>,
	multi_threading: Entity<Switch>,
}

impl Spike {
	fn new(cx: &mut Context<Self>) -> Self {
		let webp = WebpSettings::default();
		let mut subscriptions = Vec::new();

		// One slider bound to one integer field. `set` writes the field; the settings model
		// validates ranges, so the slider's range is the field's documented range.
		let mut int_slider = |cx: &mut Context<Self>, id: &'static str, label: &'static str, value: u8, range: std::ops::RangeInclusive<f32>, set: fn(&mut WebpSettings, u8)| {
			let entity = cx.new(|_| slider(id, f32::from(value), range).label(label).step(1.0).show_value(true));
			subscriptions.push(cx.subscribe(&entity, move |this: &mut Self, _, event: &SliderChanged, cx| {
				// The slider is stepped to whole numbers inside a `u8` range.
				#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
				set(&mut this.job.webp, event.value.round() as u8);
				this.settings_changed(cx);
			}));
			entity
		};
		let quality = cx.new(|_| slider("quality", webp.quality, 0.0..=100.0).label("Quality").step(1.0).show_value(true));
		let method = int_slider(cx, "method", "Method (speed ↔ size)", webp.method, 0.0..=6.0, |w, v| w.method = v);
		let alpha_quality = int_slider(cx, "alpha-quality", "Alpha quality", webp.alpha_quality, 0.0..=100.0, |w, v| w.alpha_quality = v);
		let sns = int_slider(cx, "sns", "Spatial noise shaping", webp.sns, 0.0..=100.0, |w, v| w.sns = v);
		let filter_strength = int_slider(cx, "filter-strength", "Filter strength", webp.filter_strength, 0.0..=100.0, |w, v| w.filter_strength = v);
		let filter_sharpness = int_slider(cx, "filter-sharpness", "Filter sharpness", webp.filter_sharpness, 0.0..=7.0, |w, v| w.filter_sharpness = v);
		let segments = int_slider(cx, "segments", "Segments", webp.segments, 1.0..=4.0, |w, v| w.segments = v);
		let passes = int_slider(cx, "passes", "Entropy passes", webp.passes, 1.0..=10.0, |w, v| w.passes = v);
		let near_lossless = int_slider(cx, "near-lossless", "Near-lossless (100 = off)", webp.near_lossless, 0.0..=100.0, |w, v| w.near_lossless = v);

		let mut toggle = |cx: &mut Context<Self>, id: &'static str, label: &'static str, on: bool, set: fn(&mut WebpSettings, bool)| {
			let entity = cx.new(|_| switch(id, on).label(label));
			subscriptions.push(cx.subscribe(&entity, move |this: &mut Self, _, event: &SwitchChanged, cx| {
				set(&mut this.job.webp, event.on);
				this.settings_changed(cx);
			}));
			entity
		};
		let lossless = toggle(cx, "lossless", "Lossless", webp.lossless, |w, on| w.lossless = on);
		let autofilter = toggle(cx, "autofilter", "Auto filter", webp.autofilter, |w, on| w.autofilter = on);
		let sharp_yuv = toggle(cx, "sharp-yuv", "Sharp YUV", webp.sharp_yuv, |w, on| w.sharp_yuv = on);
		let exact = toggle(cx, "exact", "Exact (keep RGB under transparency)", webp.exact, |w, on| w.exact = on);
		let multi_threading = toggle(cx, "multi-threading", "Multi-threaded", webp.multi_threading, |w, on| w.multi_threading = on);

		subscriptions.push(cx.subscribe(&quality, |this, _, event: &SliderChanged, cx| {
			this.job.webp.quality = event.value;
			this.settings_changed(cx);
		}));

		let filter_type = cx.new(|_| toggle_group("filter-type", vec![toggle_option(FilterType::Simple, "Simple"), toggle_option(FilterType::Strong, "Strong")]).selected_value(webp.filter_type));
		subscriptions.push(cx.subscribe(&filter_type, |this, _, event: &ToggleGroupChanged<FilterType>, cx| {
			if let Some(&value) = event.selected.first() {
				this.job.webp.filter_type = value;
				this.settings_changed(cx);
			}
		}));

		// `Select` reports through a callback rather than an event, so it holds a weak
		// handle back to this view.
		let weak = cx.weak_entity();
		let image_hint = cx.new(|_| {
			let weak = weak.clone();
			SelectState::new(
				select("image-hint", "Image hint", vec![(ImageHint::Default, "Default"), (ImageHint::Picture, "Picture"), (ImageHint::Photo, "Photo"), (ImageHint::Graph, "Graph")])
					.selected(webp.image_hint)
					.full_width(true)
					.on_change(move |value, _, cx| {
						weak.update(cx, |this, cx| {
							this.job.webp.image_hint = value;
							this.settings_changed(cx);
						})
						.ok();
					}),
			)
		});
		let alpha_filtering = cx.new(|_| {
			let weak = weak.clone();
			SelectState::new(
				select("alpha-filtering", "Alpha filtering", vec![(AlphaFiltering::Off, "None"), (AlphaFiltering::Fast, "Fast"), (AlphaFiltering::Best, "Best")])
					.selected(webp.alpha_filtering)
					.full_width(true)
					.on_change(move |value, _, cx| {
						weak.update(cx, |this, cx| {
							this.job.webp.alpha_filtering = value;
							this.settings_changed(cx);
						})
						.ok();
					}),
			)
		});

		Self {
			job: EncodeJob::from(webp),
			loaded: None,
			encoded: None,
			error: None,
			generation: 0,
			encoding: false,
			encode_task: None,
			controls: Controls { lossless, quality, method, alpha_quality, sns, filter_strength, filter_sharpness, filter_type, autofilter, segments, passes, near_lossless, image_hint, alpha_filtering, sharp_yuv, exact, multi_threading },
			_subscriptions: subscriptions,
		}
	}

	fn settings_changed(&mut self, cx: &mut Context<Self>) {
		self.generation += 1;
		self.encode(cx);
		cx.notify();
	}

	/// Encode the loaded source with the current settings, after [`DEBOUNCE`], on the
	/// background executor.
	fn encode(&mut self, cx: &mut Context<Self>) {
		let Some(loaded) = &self.loaded else { return };
		let image = Arc::clone(&loaded.image);
		let job = self.job.clone();
		let generation = self.generation;
		self.encoding = true;
		self.encode_task = Some(cx.spawn(async move |this, cx| {
			cx.background_executor().timer(DEBOUNCE).await;
			let result = cx.background_spawn(async move { encode_preview(&job, &image) }).await;
			this.update(cx, |this, cx| {
				if this.generation != generation {
					return;
				}
				this.encoding = false;
				match result {
					Ok(encoded) => {
						this.encoded = Some(encoded);
						this.error = None;
					}
					Err(error) => this.error = Some(error.into()),
				}
				cx.notify();
			})
			.ok();
		}));
	}

	fn open(&mut self, cx: &mut Context<Self>) {
		let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Open".into()) });
		cx.spawn(async move |this, cx| {
			let Ok(Ok(Some(paths))) = paths.await else { return };
			let Some(path) = paths.into_iter().next() else { return };
			this.update(cx, |this, cx| this.open_path(path, cx)).ok();
		})
		.detach();
	}

	/// Load `path` as the source and encode it.
	fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
		cx.spawn(async move |this, cx| {
			let loaded = cx.background_spawn(async move { load(path) }).await;
			this.update(cx, |this, cx| {
				match loaded {
					Ok(loaded) => {
						this.loaded = Some(loaded);
						this.encoded = None;
						this.error = None;
						this.settings_changed(cx);
					}
					Err(error) => this.error = Some(error.into()),
				}
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn save(&mut self, cx: &mut Context<Self>) {
		let Some(loaded) = &self.loaded else { return };
		let input = loaded.path.clone();
		let directory = input.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
		let name = input.file_stem().map(|stem| format!("{}.webp", stem.to_string_lossy()));
		let target = cx.prompt_for_new_path(&directory, name.as_deref());
		let job = self.job.clone();
		cx.spawn(async move |this, cx| {
			let Ok(Ok(Some(output))) = target.await else { return };
			// The same entry point the Tauri app's convert command calls.
			let written = cx.background_spawn(async move { skidbladnir_encode::encode_file(&job, &input, &output).map_err(|error| error.to_string()) }).await;
			this.update(cx, |this, cx| {
				this.error = written.err().map(Into::into);
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn controls_panel(&self, cx: &Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let c = &self.controls;
		let section = |title: &'static str| div().pt(px(12.)).text_xs().text_color(theme.fg_muted()).child(title);
		let labelled = |label: &'static str, control: gpui::AnyElement| v_stack().gap(px(4.)).child(div().text_sm().text_color(theme.fg_muted()).child(label)).child(control);
		v_stack()
			.id("controls")
			.w(px(320.))
			.flex_none()
			.h_full()
			.p(px(16.))
			.gap(px(10.))
			.overflow_y_scroll()
			.border_r_1()
			.border_color(theme.border())
			.bg(theme.surface())
			.child(section("COMPRESSION"))
			.child(c.lossless.clone())
			.child(c.quality.clone())
			.child(c.method.clone())
			.child(c.near_lossless.clone())
			.child(labelled("Image hint", c.image_hint.clone().into_any_element()))
			.child(section("FILTERING"))
			.child(c.autofilter.clone())
			.child(c.filter_strength.clone())
			.child(c.filter_sharpness.clone())
			.child(labelled("Filter type", c.filter_type.clone().into_any_element()))
			.child(section("ANALYSIS"))
			.child(c.sns.clone())
			.child(c.segments.clone())
			.child(c.passes.clone())
			.child(c.sharp_yuv.clone())
			.child(section("ALPHA"))
			.child(c.alpha_quality.clone())
			.child(labelled("Alpha filtering", c.alpha_filtering.clone().into_any_element()))
			.child(c.exact.clone())
			.child(section("PERFORMANCE"))
			.child(c.multi_threading.clone())
	}

	fn preview(&self, cx: &Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let pane = |title: SharedString, texture: Option<Arc<RenderImage>>| {
			v_stack()
				.flex_1()
				.min_w_0()
				.gap(px(6.))
				.child(div().text_sm().text_color(theme.fg_muted()).child(title))
				.child(
					div()
						.flex_1()
						.min_h_0()
						.rounded(px(8.))
						.border_1()
						.border_color(theme.border())
						.bg(theme.surface_secondary())
						.overflow_hidden()
						.when_some(texture, |pane, texture| pane.child(img(texture).size_full().object_fit(ObjectFit::Contain))),
				)
		};

		let Some(loaded) = &self.loaded else {
			return div().flex_1().flex().items_center().justify_center().text_color(theme.fg_muted()).child("Open an image to preview the encode.").into_any_element();
		};
		let original = format!("Original · {} · {}×{}", format_bytes(loaded.bytes), loaded.image.width, loaded.image.height);
		let encoded_title = match &self.encoded {
			Some(encoded) => {
				#[allow(clippy::cast_precision_loss)]
				let saving = 100.0 - encoded.bytes as f64 / loaded.bytes.max(1) as f64 * 100.0;
				format!("WebP · {} · {saving:.1}% smaller · {} ms", format_bytes(encoded.bytes as u64), encoded.took.as_millis())
			}
			None => "WebP · encoding…".to_owned(),
		};
		h_stack()
			.flex_1()
			.min_h_0()
			.gap(px(12.))
			.child(pane(original.into(), Some(Arc::clone(&loaded.texture))))
			.child(pane(encoded_title.into(), self.encoded.as_ref().map(|encoded| Arc::clone(&encoded.image))))
			.into_any_element()
	}
}

impl Render for Spike {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();
		let status: SharedString = match (&self.error, self.encoding) {
			(Some(error), _) => error.clone(),
			(None, true) => "Encoding…".into(),
			(None, false) => self.loaded.as_ref().map_or_else(|| "No image".into(), |loaded| loaded.path.display().to_string().into()),
		};
		h_stack()
			.size_full()
			.bg(theme.bg())
			.text_color(theme.fg())
			.child(self.controls_panel(cx))
			.child(
				v_stack()
					.flex_1()
					.min_w_0()
					.h_full()
					.p(px(16.))
					.gap(px(12.))
					.child(
						h_stack()
							.gap(px(8.))
							.items_center()
							.child(button("open", "Open image…").on_click(cx.listener(|this, _, _, cx| this.open(cx))))
							.child(button("save", "Save as WebP…").disabled(self.loaded.is_none()).on_click(cx.listener(|this, _, _, cx| this.save(cx))))
							.child(div().flex_1().min_w_0().truncate().text_xs().text_color(if self.error.is_some() { theme.danger() } else { theme.fg_muted() }).child(status)),
					)
					.child(self.preview(cx)),
			)
	}
}

/// Read and decode `path`, and make its texture.
fn load(path: PathBuf) -> Result<Loaded, String> {
	let bytes = std::fs::metadata(&path).map_err(|error| error.to_string())?.len();
	let image = source::load(&path).map_err(|error| error.to_string())?;
	let texture = texture(image.width, image.height, image.pixels.clone())?;
	Ok(Loaded { path, image: Arc::new(image), bytes, texture })
}

/// Encode exactly as a conversion would, then decode the result with libwebp for display.
///
/// The Tauri app re-wraps both sides as lossless WebP and base64 so a webview can show
/// them; here the decoded pixels go straight to the GPU.
fn encode_preview(job: &EncodeJob, image: &SourceImage) -> Result<Encoded, String> {
	let started = Instant::now();
	let encoded = encode_source_with_progress(job, image, &mut |_| true).map_err(|error| error.to_string())?;
	let bytes = encoded.len();
	let decoded = source::decode(Path::new("preview.webp"), encoded).map_err(|error| error.to_string())?;
	let image = texture(decoded.width, decoded.height, decoded.pixels)?;
	Ok(Encoded { bytes, image, took: started.elapsed() })
}

/// RGBA pixels as a gpui texture, which is BGRA.
fn texture(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Arc<RenderImage>, String> {
	for pixel in pixels.as_chunks_mut::<4>().0 {
		pixel.swap(0, 2);
	}
	let buffer = image::RgbaImage::from_raw(width, height, pixels).ok_or("the decoded buffer does not match its dimensions")?;
	Ok(Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])))
}

fn format_bytes(bytes: u64) -> String {
	#[allow(clippy::cast_precision_loss)]
	let bytes = bytes as f64;
	if bytes >= 1024.0 * 1024.0 { format!("{:.2} MB", bytes / 1024.0 / 1024.0) } else { format!("{:.1} KB", bytes / 1024.0) }
}

fn main() {
	Application::with_platform(gpui_platform::current_platform(false)).with_assets(gpuikit::assets()).run(|cx: &mut App| {
		gpuikit::init(cx);
		let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
		cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: Some(TitlebarOptions { title: Some("Skidbladnir — gpuikit spike".into()), ..Default::default() }), ..Default::default() }, |_, cx| {
			cx.new(|cx| {
				let mut spike = Spike::new(cx);
				// An image path on the command line opens straight onto it, skipping the dialog.
				if let Some(path) = std::env::args_os().nth(1) {
					spike.open_path(path.into(), cx);
				}
				spike
			})
		})
			.expect("could not open the window");
		cx.activate(true);
	});
}
