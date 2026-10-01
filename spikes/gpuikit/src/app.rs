//! The conversion window, rebuilt from `frontend/pages/index.vue`: the same state, the same
//! layout and the same behaviour, drawn by gpui instead of a webview.

use std::{
	cell::RefCell, collections::HashMap, path::{Path, PathBuf}, rc::Rc, sync::{
		Arc, atomic::{AtomicBool, Ordering}
	}
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui::{
	AnyElement, AppContext, Bounds, ClickEvent, Context, Entity, ExternalPaths, FocusHandle, Focusable, RenderImage, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, ParentElement, PathPromptOptions, Pixels, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription, Window, canvas, div, img, point, prelude::FluentBuilder, px, size, svg
};
use gpuikit::{elements::input::input, input::InputState};

use crate::css_text::css_text;
use skidbladnir_encode::{
	EncodeJob, OutputFormat, settings::HEIC_X265, source::{self, Conversion, FoundImage, PathInspection}
};

use crate::{
	controls::{Press, Ring, SliderDrag, icon}, shell::{self, preferences::PreferencesFallback, presets::Preset}, theme::{ACCENT, ACCENT_TEXT, BG, BRIGHT, BROWN, DIM, ERROR, FG, FIELD, ON_VIOLET, RULE, Type, VIOLET, WARNING, c, ca}
};

/// The version the window reports: the spike stands in for the build installed here,
/// Skidbladnir-GPL 1.1.0.
pub const APP_VERSION: &str = "1.1.0";

/// What is open over the window.
#[derive(Clone, PartialEq, Eq)]
pub enum Popup {
	Settings,
	QueueMenu,
	PresetForm,
	Select(SharedString),
}

/// One converted file, as the Tauri `ConversionReport`.
pub struct Report {
	pub output_path: PathBuf,
	pub conversion: Conversion,
	pub saving_percent: Option<f64>,
}

/// A finished preview: the Tauri `Preview`, with both sides decoded for the GPU.
pub struct PreviewView {
	pub original: Arc<RenderImage>,
	pub encoded: Arc<RenderImage>,
	pub source_bytes: u64,
	pub encoded_bytes: u64,
	pub width: u32,
	pub height: u32,
	pub saving_percent: Option<f64>,
	pub frames: u32,
	pub aspect: f32,
}

pub struct Skid {
	pub settings: Option<EncodeJob>,
	pub backend_version: String,
	pub startup_error: String,
	pub input_paths: Vec<PathBuf>,
	pub output_directory: String,
	pub busy: bool,
	pub reports: Vec<Report>,
	pub failures: Vec<(PathBuf, String)>,
	pub validation_error: String,
	pub drop_rejected: usize,
	pub inspected: Vec<PathInspection>,
	pub scanned: Vec<FoundImage>,
	pub mirror_structure: bool,
	pub include_subfolders: bool,
	pub scanned_root: Option<PathBuf>,
	pub preferences_notice: String,
	pub current_file: Option<PathBuf>,
	pub current_percent: u32,
	pub done_count: usize,
	pub cancelling: bool,
	pub cancel: Arc<AtomicBool>,
	pub presets: Vec<Preset>,
	pub preset_error: String,
	pub active_preset: String,
	pub preset_name: Entity<InputState>,
	pub preview: Option<PreviewView>,
	pub preview_format: OutputFormat,
	pub preview_path: Option<PathBuf>,
	pub previewing: bool,
	pub preview_error: String,
	pub preview_stale: bool,
	pub sidebar_collapsed: bool,
	pub popup: Option<Popup>,
	/// The highlighted item of the open menu: set by the pointer, or by the keyboard, which
	/// starts it on the chosen item (a select) or the first (the queue menu) when the menu
	/// is opened from the keyboard, as Reka does.
	pub menu_highlight: Option<usize>,
	/// What each item of the open menu does, rebuilt as the menu renders, for Enter and Space.
	pub menu_actions: Vec<Rc<dyn Fn(&mut Skid, &mut Window, &mut Context<Skid>)>>,
	/// Set when Enter or Space chose a menu item: the key-up that follows would otherwise
	/// click the still-focused trigger and open the menu again.
	pub swallow_click: bool,
	/// With `SKID_BENCH` set: what is being timed, and since when. Reported once the frame
	/// that shows its result has been drawn.
	pub bench: Option<(&'static str, std::time::Instant)>,
	/// The WebP panel's own state: the `-preset` and `-z` pickers.
	pub webp_preset: skidbladnir_encode::settings::Preset,
	pub lossless_level: u8,
	pub webp_preset_error: String,

	pub focus: HashMap<SharedString, FocusHandle>,
	pub root_focus: FocusHandle,
	pub inputs: HashMap<SharedString, Entity<InputState>>,
	pub editing: Option<SharedString>,
	pub disclosures: HashMap<SharedString, bool>,
	pub slider_bounds: Rc<RefCell<HashMap<SharedString, Bounds<Pixels>>>>,
	pub drag: Option<SliderDrag>,
	pub scroll: ScrollHandle,
	/// Where the scrollbar thumb was grabbed, while it is being dragged.
	pub scrollbar_drag: Option<Pixels>,
	/// `UpdateBanner.vue`: the version on offer, once the manifest says there is one.
	pub update: Option<String>,
	pub update_dismissed: bool,
	pub update_error: String,
	pub subscriptions: Vec<Subscription>,
}

impl Skid {
	pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
		let directory = shell::config_directory();
		let loaded = shell::preferences::load_from(&directory);
		let preferences_notice = match loaded.fell_back {
			None | Some(PreferencesFallback::NoFile) => String::new(),
			Some(PreferencesFallback::Unreadable) => "Your saved settings could not be read, so the defaults were loaded.".to_owned(),
			Some(PreferencesFallback::Unparseable) => "Your saved settings file was damaged, so the defaults were loaded.".to_owned(),
			Some(PreferencesFallback::InvalidSettings) => "Your saved settings were out of range, so the defaults were loaded.".to_owned(),
		};
		let preset_name = cx.new(|cx| {
			let mut state = InputState::new_singleline(cx).submit_on(gpuikit::input::SubmitOn::Enter);
			state.set_placeholder("Name these settings…", cx);
			state
		});
		let mut subscriptions = Vec::new();
		subscriptions.push(cx.subscribe(&preset_name, |this, _, event: &gpuikit::input::InputStateEvent, cx| match event {
			gpuikit::input::InputStateEvent::Submit => this.save_preset(cx),
			gpuikit::input::InputStateEvent::TextChanged => cx.notify(),
			_ => {}
		}));
		let root_focus = cx.focus_handle();
		window.focus(&root_focus, cx);
		Self {
			settings: Some(loaded.preferences.settings),
			backend_version: crate::backend_version(),
			startup_error: String::new(),
			input_paths: Vec::new(),
			output_directory: loaded.preferences.output_directory.map(|path| path.display().to_string()).unwrap_or_default(),
			busy: false,
			reports: Vec::new(),
			failures: Vec::new(),
			validation_error: String::new(),
			drop_rejected: 0,
			inspected: Vec::new(),
			scanned: Vec::new(),
			mirror_structure: true,
			include_subfolders: false,
			scanned_root: None,
			preferences_notice,
			current_file: None,
			current_percent: 0,
			done_count: 0,
			cancelling: false,
			cancel: Arc::new(AtomicBool::new(false)),
			presets: shell::presets::load_from(&directory),
			preset_error: String::new(),
			active_preset: String::new(),
			preset_name,
			preview: None,
			preview_format: OutputFormat::Webp,
			preview_path: None,
			previewing: false,
			preview_error: String::new(),
			preview_stale: false,
			sidebar_collapsed: false,
			popup: None,
			menu_highlight: None,
			menu_actions: Vec::new(),
			swallow_click: false,
			bench: None,
			webp_preset: skidbladnir_encode::settings::Preset::Photo,
			lossless_level: 6,
			webp_preset_error: String::new(),
			focus: HashMap::new(),
			root_focus,
			inputs: HashMap::new(),
			editing: None,
			disclosures: HashMap::new(),
			slider_bounds: Rc::new(RefCell::new(HashMap::new())),
			drag: None,
			scroll: ScrollHandle::new(),
			scrollbar_drag: None,
			update: None,
			update_dismissed: false,
			update_error: String::new(),
			subscriptions,
		}
		.checking_for_update(cx)
		.debug_state(cx)
	}

	/// Debugging aids for comparing the two windows state by state: `SKID_FORMAT` picks the
	/// output format (not saved), `SKID_INPUTS` queues files as a drop would, `SKID_PREVIEW`
	/// runs the preview, `SKID_OPEN` opens a pop-up (`settings`, `queue`,
	/// `preset-form`, or a select's id such as `select-webp-preset`), `SKID_SCROLL` scrolls
	/// the settings column to that many pixels.
	fn debug_state(mut self, cx: &mut Context<Self>) -> Self {
		if let Ok(format) = std::env::var("SKID_FORMAT") {
			self.job().format = match format.as_str() {
				"avif" => OutputFormat::Avif,
				"jxl" => OutputFormat::Jxl,
				"heic" => OutputFormat::Heic,
				_ => OutputFormat::Webp,
			};
		}
		if let Ok(inputs) = std::env::var("SKID_INPUTS") {
			let paths: Vec<PathBuf> = std::env::split_paths(&inputs).collect();
			self.dropped(&ExternalPaths(paths.into()), cx);
		}
		// The benchmark starts these 1.5s after launch, as it presses them in the old window.
		let preview = std::env::var_os("SKID_PREVIEW").is_some();
		let convert = std::env::var_os("SKID_CONVERT").is_some();
		if preview || convert {
			cx.spawn(async move |this, cx| {
				cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
				this.update(cx, |this, cx| {
					if preview {
						this.run_preview(cx);
					}
					if convert {
						this.convert(cx);
					}
				})
				.ok();
			})
			.detach();
		}
		let open = std::env::var("SKID_OPEN").ok();
		let scroll = std::env::var("SKID_SCROLL").ok().and_then(|value| value.parse::<f32>().ok());
		if open.is_some() || scroll.is_some() {
			// After the window has settled, so the triggers' bounds are known.
			cx.spawn(async move |this, cx| {
				cx.background_executor().timer(std::time::Duration::from_millis(if std::env::var_os("SKID_PREVIEW").is_some() { 6000 } else { 1500 })).await;
				this.update(cx, |this, cx| {
					if let Some(offset) = scroll {
						this.scroll.set_offset(point(px(0.), px(-offset)));
					}
					this.popup = open.map(|open| match open.as_str() {
						"settings" => Popup::Settings,
						"queue" => Popup::QueueMenu,
						"preset-form" => Popup::PresetForm,
						other => Popup::Select(other.to_owned().into()),
					});
					cx.notify();
				})
				.ok();
			})
			.detach();
		}
		self
	}

	/// `check_for_update`: the same manifest, the same opt-out.
	fn checking_for_update(self, cx: &mut Context<Self>) -> Self {
		if std::env::var_os("SKID_PROBE").is_some() {
			// Probes print at the start of the next frame, so ask for one once the window has
			// settled at its final size.
			cx.spawn(async move |this, cx| {
				cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
				this.update(cx, |_, cx| cx.notify()).ok();
			})
			.detach();
		}
		if std::env::var_os("SKIDBLADNIR_NO_UPDATE_CHECK").is_none_or(|value| value.is_empty()) {
			cx.spawn(async move |this, cx| {
				let offered = cx.background_spawn(async move { crate::updater::check(APP_VERSION) }).await;
				if let Some(version) = offered {
					this.update(cx, |this, cx| {
						this.update = Some(version);
						cx.notify();
					})
					.ok();
				}
			})
			.detach();
		}
		self
	}

	fn update_banner(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
		let version = self.update.as_ref().filter(|_| !self.update_dismissed)?;
		let description = if self.update_error.is_empty() { format!("You have {APP_VERSION}. Installing restarts Skidbladnir.") } else { format!("The update could not be installed: {}", self.update_error) };
		Some(
			div()
				.flex()
				.w_full()
				.items_center()
				.gap(px(10.))
				.p(px(16.))
				.rounded(px(12.))
				.bg(ca(ACCENT, 0.1))
				.border_1()
				.border_color(ca(ACCENT, 0.25))
				.text_color(c(ACCENT))
				.child(icon("lucide--download", 20., c(ACCENT)))
				.child(div().min_w_0().flex_1().flex().flex_col().child(div().sm().medium().child(css_text(format!("Skidbladnir {version} is available")))).child(div().sm().opacity(0.9).mt(px(4.)).child(css_text(description))))
				.child(
					div()
						.flex()
						.flex_none()
						.flex_wrap()
						.items_center()
						.gap(px(6.))
						.child(
							div()
								.id("update-install")
								.flex()
								.items_center()
								.gap(px(4.))
								.px(px(8.))
								.py(px(4.))
								.rounded(px(9.))
								.bg(c(ACCENT))
								.hover(|button| button.bg(ca(ACCENT, 0.75)))
								.xs()
								.medium()
								.text_color(c(BG))
								.child(if self.update_error.is_empty() { "Install and restart" } else { "Try again" })
								.press(self, "update-install", Ring::Primary, 9., |this, _, cx|  {
									// Downloading, verifying and swapping the binary is
									// tauri-plugin-updater's job; the spike has no replacement for it.
									this.update_error = "installing is not implemented in the gpuikit spike".to_owned();
									cx.notify();
								}, window, cx),
						)
						.child(
							div()
								.id("update-dismiss")
								.rounded(px(9.))
								.press(self, "update-dismiss", Ring::Neutral, 9., |this, _, cx|  {
									this.update_dismissed = true;
									cx.notify();
								}, window, cx)
								.child(icon("lucide--x", 20., c(DIM))),
						),
				)
				.into_any_element(),
		)
	}

	pub fn job(&mut self) -> &mut EncodeJob {
		self.settings.get_or_insert_with(EncodeJob::default)
	}

	/// After any settings edit: what `index.vue`'s two deep watchers do.
	pub fn changed(&mut self, cx: &mut Context<Self>) {
		if self.preview.is_some() {
			self.preview_stale = true;
		}
		if !self.active_preset.is_empty() {
			let stored = self.presets.iter().find(|preset| preset.name == self.active_preset).map(|preset| &preset.settings);
			if stored != self.settings.as_ref() {
				self.active_preset.clear();
			}
		}
		cx.notify();
	}

	fn set_inputs(&mut self, paths: Vec<PathBuf>) {
		self.preview = None;
		self.preview_error.clear();
		self.preview_path = paths.first().cloned();
		self.input_paths = paths;
	}

	fn choose_inputs(&mut self, cx: &mut Context<Self>) {
		// gpui's prompt has no file-type filter; the old dialog lists only images.
		let picked = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
		cx.spawn(async move |this, cx| {
			let Ok(Ok(Some(paths))) = picked.await else { return };
			let inspected = cx.background_spawn({
				let paths = paths.clone();
				async move { source::inspect_paths(&paths) }
			});
			let inspected = inspected.await;
			this.update(cx, |this, cx| {
				this.scanned.clear();
				this.scanned_root = None;
				this.set_inputs(paths);
				this.inspected = inspected;
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn choose_folder(&mut self, cx: &mut Context<Self>) {
		let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
		cx.spawn(async move |this, cx| {
			let Ok(Ok(Some(paths))) = picked.await else { return };
			let Some(root) = paths.into_iter().next() else { return };
			this.update(cx, |this, cx| {
				this.scanned_root = Some(root);
				this.scan_queued_folder(cx);
			})
			.ok();
		})
		.detach();
	}

	fn scan_queued_folder(&mut self, cx: &mut Context<Self>) {
		let Some(root) = self.scanned_root.clone() else { return };
		let recursive = self.include_subfolders;
		cx.spawn(async move |this, cx| {
			let (found, inspected) = cx
				.background_spawn(async move {
					let found = source::scan_directory(&root, recursive);
					let paths: Vec<PathBuf> = found.iter().map(|entry| entry.path.clone()).collect();
					let inspected = source::inspect_paths(&paths);
					(found, inspected)
				})
				.await;
			this.update(cx, |this, cx| {
				this.set_inputs(found.iter().map(|entry| entry.path.clone()).collect());
				this.scanned = found;
				this.inspected = inspected;
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	pub fn set_include_subfolders(&mut self, include: bool, cx: &mut Context<Self>) {
		self.include_subfolders = include;
		if self.scanned_root.is_some() && !self.busy {
			self.scan_queued_folder(cx);
		}
		cx.notify();
	}

	fn choose_output(&mut self, cx: &mut Context<Self>) {
		let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
		cx.spawn(async move |this, cx| {
			let Ok(Ok(Some(paths))) = picked.await else { return };
			let Some(directory) = paths.into_iter().next() else { return };
			this.update(cx, |this, cx| {
				this.output_directory = directory.display().to_string();
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn dropped(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
		let dropped = source::inspect_paths(paths.paths());
		let usable: Vec<PathInspection> = dropped.iter().filter(|entry| entry.supported).cloned().collect();
		self.drop_rejected = dropped.len() - usable.len();
		if !usable.is_empty() {
			self.scanned.clear();
			self.scanned_root = None;
			self.set_inputs(usable.iter().map(|entry| entry.path.clone()).collect());
			self.inspected = usable;
		}
		cx.notify();
	}

	fn can_convert(&self) -> bool {
		self.settings.is_some() && !self.busy && !self.input_paths.is_empty() && !self.output_directory.is_empty()
	}

	fn convert(&mut self, cx: &mut Context<Self>) {
		let Some(settings) = self.settings.clone() else { return };
		self.bench = Some(("convert", std::time::Instant::now()));
		self.busy = true;
		self.cancelling = false;
		self.reports.clear();
		self.failures.clear();
		self.validation_error.clear();
		self.done_count = 0;
		self.current_percent = 0;
		self.current_file = None;
		if let Err(error) = settings.validate() {
			self.validation_error = error.to_string();
			self.busy = false;
			cx.notify();
			return;
		}
		let inputs = self.input_paths.clone();
		let scanned = self.scanned.clone();
		let mirror = self.mirror_structure;
		let output = PathBuf::from(&self.output_directory);
		let cancel = Arc::clone(&self.cancel);
		cx.notify();
		cx.spawn(async move |this, cx| {
			for input in inputs {
				cancel.store(false, Ordering::Relaxed);
				let (sender, mut receiver) = futures::channel::mpsc::unbounded::<u32>();
				let job = settings.clone();
				let cancel = Arc::clone(&cancel);
				let found = scanned.iter().find(|entry| entry.path == input).map(|entry| entry.relative.clone());
				let output = output.clone();
				let file = input.clone();
				this.update(cx, |this, cx| {
					this.current_file = Some(file);
					cx.notify();
				})
				.ok();
				let run = cx.background_spawn({
					let input = input.clone();
					async move {
						let mut on_progress = |percent: u32| {
							sender.unbounded_send(percent).ok();
							!cancel.load(Ordering::Relaxed)
						};
						// What `convert_scanned` and `convert_image` do.
						let output_path = match (found, mirror) {
							(Some(relative), true) => {
								let Some(path) = source::mirrored_output_path(&output, &relative, job.format) else {
									return Err(format!("refusing to write `{}`: it would land outside the chosen folder", relative.display()));
								};
								if let Some(parent) = path.parent() {
									std::fs::create_dir_all(parent).map_err(|error| format!("could not create `{}`: {error}", parent.display()))?;
								}
								path
							}
							_ => source::output_path_in(output, &input, job.format),
						};
						let conversion = source::encode_file_with_progress(&job, &input, &output_path, &mut on_progress).map_err(|error| error.to_string())?;
						Ok(Report { output_path, conversion, saving_percent: conversion.saving_percent() })
					}
				});
				let forward = {
					let this = this.clone();
					let mut cx = cx.clone();
					async move {
						use futures::StreamExt as _;
						while let Some(percent) = receiver.next().await {
							this.update(&mut cx, |this, cx| {
								this.current_percent = percent;
								cx.notify();
							})
							.ok();
						}
					}
				};
				let (result, ()) = futures::join!(run, forward);
				let stop = this
					.update(cx, |this, cx| {
						let stop = match result {
							Ok(report) => {
								this.reports.push(report);
								false
							}
							Err(message) if message.contains("cancelled") => true,
							Err(message) => {
								this.failures.push((input.clone(), message));
								false
							}
						};
						this.done_count += 1;
						cx.notify();
						stop
					})
					.unwrap_or(true);
				if stop {
					break;
				}
			}
			this.update(cx, |this, cx| {
				this.busy = false;
				this.cancelling = false;
				this.current_file = None;
				let preferences = shell::preferences::Preferences { settings: this.settings.clone().unwrap_or_default(), output_directory: (!this.output_directory.is_empty()).then(|| PathBuf::from(&this.output_directory)) };
				let _ = shell::preferences::save_to(&shell::config_directory(), &preferences);
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn save_preset(&mut self, cx: &mut Context<Self>) {
		let Some(settings) = self.settings.clone() else { return };
		let name = self.preset_name.read(cx).content().to_owned();
		if name.trim().is_empty() {
			return;
		}
		self.preset_error.clear();
		match shell::presets::save_to(&shell::config_directory(), &name, &settings) {
			Ok(presets) => {
				self.presets = presets;
				self.active_preset = name.trim().to_owned();
				self.preset_name.update(cx, |state, cx| state.set_content("", cx));
				self.popup = None;
			}
			Err(error) => self.preset_error = error.to_string(),
		}
		cx.notify();
	}

	fn run_preview(&mut self, cx: &mut Context<Self>) {
		let (Some(settings), Some(path)) = (self.settings.clone(), self.preview_path.clone()) else { return };
		self.bench = Some(("preview", std::time::Instant::now()));
		self.previewing = true;
		self.preview_error.clear();
		cx.notify();
		cx.spawn(async move |this, cx| {
			let format = settings.format;
			// The Tauri app's own preview, unchanged, so both windows do the same work. Its two
			// `data:` URLs are decoded here instead of by a webview.
			let result = cx
				.background_spawn(async move {
					let started = std::time::Instant::now();
					let preview = shell::preview::preview(&settings, &path)?;
					let encoded_at = started.elapsed();
					let decode = |url: &str| STANDARD.decode(url.trim_start_matches("data:image/webp;base64,")).map_err(|error| error.to_string());
					let original = decode(&preview.original)?;
					let aspect = image_aspect(&original).unwrap_or(1.0);
					// Decoded to pixels here, every frame of an animation included, so the frame
					// that shows the preview has nothing left to decode — as the webview has
					// decoded both `<img>`s by the time they appear.
					let original_texture = texture(&original)?;
					let encoded_texture = texture(&decode(&preview.encoded)?)?;
					if std::env::var_os("SKID_BENCH").is_some() {
						eprintln!("bench preview_breakdown encode_ms {} decode_ms {}", encoded_at.as_millis(), (started.elapsed() - encoded_at).as_millis());
					}
					Ok::<_, String>(PreviewView { original: original_texture, encoded: encoded_texture, source_bytes: preview.source_bytes, encoded_bytes: preview.encoded_bytes, width: preview.width, height: preview.height, saving_percent: preview.saving_percent, frames: preview.frames, aspect })
				})
				.await;
			this.update(cx, |this, cx| {
				this.previewing = false;
				match result {
					Ok(preview) => {
						this.preview = Some(preview);
						this.preview_format = format;
						this.preview_stale = false;
					}
					Err(error) => {
						this.preview = None;
						this.preview_error = error;
					}
				}
				cx.notify();
			})
			.ok();
		})
		.detach();
	}

	fn queue_folder(&self) -> String {
		if self.input_paths.is_empty() {
			return "Nothing queued".to_owned();
		}
		let folders: Vec<PathBuf> = self.input_paths.iter().map(|path| path.parent().map(Path::to_path_buf).unwrap_or_default()).collect();
		let mut common = folders[0].clone();
		for folder in &folders {
			while !folder.starts_with(&common) {
				if !common.pop() {
					break;
				}
			}
		}
		if common.as_os_str().is_empty() { folders[0].display().to_string() } else { common.display().to_string() }
	}

	fn file_count(&self) -> String {
		format!("{} file{}", self.input_paths.len(), if self.input_paths.len() == 1 { "" } else { "s" })
	}

	// ---- The frame --------------------------------------------------------------------

	fn rail(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let format = self.settings.as_ref().map(|settings| settings.format);
		let mut items = div().flex().flex_col().gap(px(16.));
		if let Some(format) = format {
			for (value, name, label) in [(OutputFormat::Webp, "iconoir--webp-format", "WebP"), (OutputFormat::Avif, "vscode-icons--file-type-avif", "AVIF"), (OutputFormat::Jxl, "skid--jxl-format", "JPEG XL"), (OutputFormat::Heic, "bi--filetype-heic", "HEIC")] {
				let checked = value == format;
				// One tab stop for the group, the chosen format; the arrows move between them.
				let focus = self.focus_handle_for(&format!("format-{label}"), cx).tab_stop(checked);
				let ring = Self::ring_visible(&focus, window);
				// The AVIF mark is multicoloured: gpui's `svg()` paints a single tint, so it is
				// drawn as an image instead, as the webview draws it.
				let glyph: AnyElement = if value == OutputFormat::Avif { img(format!("icons/{name}.svg")).size(px(24.)).into_any_element() } else { svg().path(format!("icons/{name}.svg")).size(px(24.)).text_color(c(FG)).into_any_element() };
				items = items.child(
					div()
						.id(SharedString::from(format!("format-{label}")))
						.relative()
						.px(px(8.))
						.py(px(4.))
						.border_l_1()
						.border_color(if checked || ring { c(ACCENT) } else { gpui::transparent_black() })
						.when(!checked && !ring, |item| item.hover(|item| item.bg(ca(FIELD, 0.5))))
						.cursor_pointer()
						.track_focus(&focus)
						.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
							this.job().format = value;
							this.changed(cx);
						}))
						.on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
							const ORDER: [(OutputFormat, &str); 4] = [(OutputFormat::Webp, "WebP"), (OutputFormat::Avif, "AVIF"), (OutputFormat::Jxl, "JPEG XL"), (OutputFormat::Heic, "HEIC")];
							let index = ORDER.iter().position(|(format, _)| *format == value).unwrap_or(0);
							let target = match event.keystroke.key.as_str() {
								"space" => index,
								"down" | "right" => (index + 1) % 4,
								"up" | "left" => (index + 3) % 4,
								_ => return,
							};
							this.job().format = ORDER[target].0;
							let handle = this.focus_handle_for(&format!("format-{}", ORDER[target].1), cx);
							window.focus(&handle, cx);
							this.changed(cx);
							cx.stop_propagation();
						}))
						.child(glyph)
						.when(ring, |item| item.child(crate::controls::ring(3., ca(ACCENT, 0.25), 0., 0.))),
				);
			}
		}
		let settings_open = self.popup == Some(Popup::Settings);
		let gear = div()
			.id("app-settings")
			.child(self.record("app-settings"))
			.relative()
			.px(px(8.))
			.py(px(4.))
			.rounded(px(9.))
			.hover(|button| button.bg(c(FIELD)))
			.press(self, "app-settings", Ring::Neutral, 9., |this, window, cx|  {
				this.popup = if this.popup == Some(Popup::Settings) { None } else { Some(Popup::Settings) };
				if this.popup.is_some() {
					let first = this.focus_handle_for("popover-include", cx);
					window.focus(&first, cx);
				}
				cx.notify();
			}, window, cx)
			.child(icon("iconamoon--settings-light", 24., c(FG)))
			.when(settings_open, |gear| gear.child(self.settings_popover(window, cx)));
		div()
			.id("rail")
			.relative()
			.child(crate::controls::probe("nav"))
			.flex()
			.flex_none()
			.flex_col()
			.items_center()
			.gap(px(8.))
			.px(px(16.))
			.pt(px(32.))
			.pb(px(16.))
			.on_mouse_down(MouseButton::Left, |event, window, _| drag_window(event, window))
			.child(img("skidbladnir-logo.svg").w(px(55.)).h(px(64.)))
			.child(div().flex().flex_1().flex_col().items_start().gap(px(16.)).px(px(8.)).pt(px(32.)).pb(px(16.)).child(items).child(div().flex_1()).child(gear))
			.into_any_element()
	}

	fn settings_popover(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let include = self.include_subfolders;
		let mirror = self.mirror_structure;
		let toggles = vec![
			self.toggle("popover-include", "Include subfolders", Some("Choosing a folder also takes every folder inside it. Off takes only the images directly in it."), include, false, |this, on| this.include_subfolders = on, window, cx),
			self.toggle("popover-mirror", "Recreate folder structure", Some("When subfolders are included, mirror them in the destination. Off writes every file side by side."), mirror, !include, |this, on| this.mirror_structure = on, window, cx),
		];
		let version = self.backend_version.clone();
		let gear = self.bounds_of("app-settings").unwrap_or_default();
		let body = div().flex().flex_col().gap(px(8.)).p(px(8.)).xs().children(toggles).when(!version.is_empty(), |body| body.child(div().px(px(10.)).pb(px(4.)).text_color(c(DIM)).child(css_text(version))));
		crate::controls::place(gpui::Anchor::BottomLeft, point(gear.right() + px(8.), gear.bottom()), crate::controls::popup_panel("settings-popover", Some(px(288.)), body, cx)).into_any_element()
	}

	fn sidebar(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let queue_open = self.popup == Some(Popup::QueueMenu);
		let preset_open = self.popup == Some(Popup::PresetForm);
		let count = self.input_paths.len();
		let mut presets = div().flex().flex_col().gap(px(2.));
		for (index, preset) in self.presets.clone().iter().enumerate() {
			let active = self.active_preset == preset.name;
			let name = preset.name.clone();
			let delete_name = preset.name.clone();
			let group = SharedString::from(format!("preset-{index}"));
			presets = presets.child(
				div()
					.relative()
					.group(group.clone())
					.child(
						div()
							.id(SharedString::from(format!("preset-apply-{index}")))
							.relative()
							.child(crate::controls::probe(&format!("button {}", preset.name)))
							.w_full()
							.truncate()
							.border_l_1()
							.px(px(10.))
							.py(px(7.))
							.pr(px(32.))
							.xs()
							.when(active, |item| item.border_color(c(BROWN)).semibold().text_color(c(ACCENT_TEXT)))
							.when(!active, |item| item.rounded(px(12.)).border_color(gpui::transparent_black()).text_color(c(BRIGHT)).hover(|item| item.bg(ca(FIELD, 0.7))))
							.press(self, "preset-apply", Ring::Plain(BRIGHT), 12., move |this, _, cx|  {
								if let Some(preset) = this.presets.iter().find(|preset| preset.name == name) {
									this.settings = Some(preset.settings.clone());
									this.active_preset = name.clone();
									this.changed(cx);
								}
							}, window, cx)
							.child(preset.name.clone()),
					)
					.child(
						div()
							.id(SharedString::from(format!("preset-delete-{index}")))
							.absolute()
							.top(px(4.))
							.right(px(4.))
							.p(px(4.))
							.rounded(px(9.))
							.opacity(0.)
							.group_hover(group, |button| button.opacity(1.))
							.hover(|button| button.bg(c(FIELD)))
							.press(self, "preset-delete", Ring::Neutral, 9., move |this, _, cx|  {
								this.preset_error.clear();
								match shell::presets::delete_from(&shell::config_directory(), &delete_name) {
									Ok(presets) => {
										this.presets = presets;
										if this.active_preset == delete_name {
											this.active_preset.clear();
										}
									}
									Err(error) => this.preset_error = error.to_string(),
								}
								cx.notify();
							}, window, cx)
							.child(icon("lucide--x", 14., c(FG))),
					),
			);
		}
		let name_empty = self.preset_name.read(cx).content().trim().is_empty();
		let add = self.bounds_of("preset-add").unwrap_or_default();
		let preset_form = preset_open.then(|| {
			crate::controls::place(gpui::Anchor::TopLeft, point(add.right() + px(8.), add.top()), crate::controls::popup_panel("preset-form", None,
				div()
					.flex()
					.w(px(256.))
					.gap(px(8.))
					.p(px(8.))
					.child(input(&self.preset_name, cx).flex_1().h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(BG)).border_1().border_color(c(RULE)).xs().text_color(c(FG)))
					.child(primary_button("preset-save", "Save", name_empty).press(self, "preset-save", Ring::Primary, 9., |this, _, cx|  this.save_preset(cx), window, cx)),
				cx,
			))
		});
		div()
			.flex()
			.flex_none()
			.flex_col()
			.w(px(272.))
			.gap(px(12.))
			.p(px(16.))
			.child(
				div()
					.id("collapse-row")
					.flex()
					.justify_end()
					.on_mouse_down(MouseButton::Left, |event, window, _| drag_window(event, window))
					.child(link_button("collapse", "Collapse", 90.).press(self, "collapse", Ring::Neutral, 9., |this, _, cx|  {
						this.sidebar_collapsed = true;
						cx.notify();
					}, window, cx)),
			)
			.child(
				div()
					.relative()
					.child(
						sidebar_button("queue", "el--inbox-box", "Queue", &self.queue_folder(), (count > 0).then(|| count.to_string())).child(self.record("queue")).press(self, "queue", Ring::Plain(FG), 12., |this, window, cx|  {
							this.popup = if this.popup == Some(Popup::QueueMenu) { None } else { Some(Popup::QueueMenu) };
							this.menu_highlight = window.last_input_was_keyboard().then_some(0);
							cx.notify();
						}, window, cx),
					)
					.when(queue_open, |queue| queue.child(self.queue_menu(cx))),
			)
			.child(sidebar_button("destination", "ic--baseline-upcoming", "Destination", if self.output_directory.is_empty() { "Not chosen" } else { &self.output_directory }, None).press(self, "destination", Ring::Plain(FG), 12., |this, _, cx|  this.choose_output(cx), window, cx))
			.child(
				div()
					.flex()
					.items_center()
					.justify_between()
					.child(div().relative().child(crate::controls::probe("h2 Presets")).px11().semibold().text_color(c(BRIGHT)).child("Presets"))
					.child(
						div()
							.relative()
							.child(
								div()
									.id("preset-add")
									.child(self.record("preset-add"))
									.rounded(px(9.))
									.hover(|button| button.bg(c(FIELD)))
									.press(self, "preset-add", Ring::Neutral, 9., |this, window, cx|  {
										this.popup = if this.popup == Some(Popup::PresetForm) { None } else { Some(Popup::PresetForm) };
										if this.popup.is_some() {
											let handle = this.preset_name.read(cx).focus_handle(cx);
											window.focus(&handle, cx);
										}
										cx.notify();
									}, window, cx)
									.child(icon("material-symbols--add", 16., c(FG))),
							)
							.children(preset_form),
					),
			)
			.child(presets)
			.when(self.presets.is_empty(), |aside| aside.child(div().px(px(10.)).xs().text_color(c(DIM)).child(css_text("No saved presets yet. Set the controls how you like them and press +."))))
			.when(!self.preset_error.is_empty(), |aside| aside.child(div().px(px(10.)).xs().text_color(c(ERROR)).child(self.preset_error.clone())))
			.child(div().flex_1())
			.child(div().px10().text_color(c(BRIGHT)).child(format!("v{APP_VERSION}{}", if HEIC_X265 { " · GPL edition" } else { "" })))
			.into_any_element()
	}

	fn queue_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
		let include = self.include_subfolders;
		let trigger = self.bounds_of("queue").unwrap_or_default();
		// Menu items: `p-1.5 text-sm gap-1.5`, a 20px icon, the text, and a faint fill under the
		// highlighted item, whose text and icon turn from muted to highlighted.
		let highlight = self.menu_highlight;
		self.menu_actions = vec![
			Rc::new(|this: &mut Skid, _: &mut Window, cx: &mut Context<Skid>| {
				this.popup = None;
				this.choose_inputs(cx);
			}),
			Rc::new(|this: &mut Skid, _: &mut Window, cx: &mut Context<Skid>| {
				this.popup = None;
				this.choose_folder(cx);
			}),
			Rc::new(move |this: &mut Skid, _: &mut Window, cx: &mut Context<Skid>| this.set_include_subfolders(!include, cx)),
		];
		let item = |id: &str, index: usize, icon_name: Option<&str>| {
			let lit = highlight == Some(index);
			div()
				.id(SharedString::from(id.to_owned()))
				.relative()
				.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
					if *hovered {
						this.menu_highlight = Some(index);
						cx.notify();
					}
				}))
				.flex()
				.items_start()
				.gap(px(6.))
				.p(px(6.))
				// Nuxt UI's item ends in an empty trailing slot, which still takes a gap.
				.pr(px(12.))
				.sm()
				.text_color(c(if lit { BRIGHT } else { FG }))
				.when(lit, |item| item.child(div().absolute().top(px(1.)).left(px(1.)).right(px(1.)).bottom(px(1.)).rounded(px(9.)).bg(ca(FIELD, 0.5))))
				.when_some(icon_name.map(str::to_owned), move |item, name| item.child(div().relative().child(icon(&name, 20., c(if lit { FG } else { DIM })))))
		};
		let menu = div()
			.flex()
			.flex_col()
			.child(
				div()
					.flex()
					.flex_col()
					.p(px(4.))
					.child(item("menu-images", 0, Some("lucide--images")).child(div().relative().child("Choose images…")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
						this.popup = None;
						this.choose_inputs(cx);
					})))
					.child(item("menu-folder", 1, Some("lucide--folder-search")).child(div().relative().child("Choose a folder…")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
						this.popup = None;
						this.choose_folder(cx);
					}))),
			)
			.child(div().h(px(1.)).bg(c(RULE)))
			.child(
				div().flex().flex_col().p(px(4.)).child(
					item("menu-subfolders", 2, None)
						// Kept open, so the box can be ticked and then a folder chosen.
						.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_include_subfolders(!include, cx)))
						.child(div().relative().flex_1().child("Include subfolders"))
						.when(include, |row| row.child(icon("lucide--check", 20., c(FG)))),
				),
			);
		crate::controls::place(gpui::Anchor::TopLeft, point(trigger.left(), trigger.bottom() + px(8.)), crate::controls::popup_panel("queue-menu", None, menu, cx)).into_any_element()
	}

	fn title_bar(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let window_button = |id: &str, icon_name: &str, wide: bool| div().id(SharedString::from(id.to_owned())).flex_none().pt(px(16.)).pb(px(16.)).pl(px(16.)).pr(px(if wide { 32. } else { 16. })).hover(|button| button.bg(c(FIELD))).child(icon(icon_name, 16., c(FG)));
		div()
			.id("title-bar")
			.flex()
			.flex_none()
			.items_center()
			.gap(px(16.))
			.on_mouse_down(MouseButton::Left, |event, window, _| drag_window(event, window))
			.when(self.sidebar_collapsed, |bar| {
				bar.child(link_button("expand", "Expand", -90.).press(self, "expand", Ring::Neutral, 9., |this, _, cx|  {
					this.sidebar_collapsed = false;
					cx.notify();
				}, window, cx))
			})
			.child(div().h(px(20.)).flex_1())
			.child(
				div()
					.flex()
					.items_center()
					.child(window_button("minimise", "material-symbols--minimize", false).press(self, "minimise", Ring::Neutral, 0., |_, window, _| window.minimize_window(), window, cx))
					.child(window_button("maximise", "mdi--maximize", false).press(self, "maximise", Ring::Neutral, 0., |_, window, _| window.zoom_window(), window, cx))
					.child(window_button("close", "material-symbols--close", true).press(self, "close", Ring::Neutral, 0., |_, _, cx| cx.quit(), window, cx)),
			)
			.into_any_element()
	}

	fn header(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let text = if !self.input_paths.is_empty() && !self.output_directory.is_empty() {
			format!("{} → {}", self.file_count(), self.output_directory)
		} else if !self.input_paths.is_empty() {
			format!("Choose a destination to convert {}.", self.file_count())
		} else {
			"Drop images anywhere on the window, or choose them from the queue.".to_owned()
		};
		let enabled = self.can_convert();
		let action = if self.busy {
			div()
				.id("cancel")
				.flex()
				.flex_none()
				.items_center()
				.gap(px(8.))
				.px(px(12.))
				.py(px(8.))
				.rounded(px(9.))
				.bg(ca(ERROR, 0.1))
				.sm()
				.medium()
				.text_color(c(ERROR))
				.when(self.cancelling, |button| button.child(icon("lucide--loader-circle", 20., c(ERROR))))
				.child(if self.cancelling { "Stopping…" } else { "Cancel" })
				.press(self, "cancel", Ring::Error, 9., |this, _, cx|  {
					this.cancelling = true;
					this.cancel.store(true, Ordering::Relaxed);
					cx.notify();
				}, window, cx)
		} else {
			div()
				.id("convert")
				.flex()
				.flex_none()
				.items_center()
				.gap(px(8.))
				.px(px(16.))
				.py(px(10.))
				.rounded(px(6.))
				.bg(if enabled { c(VIOLET) } else { ca(VIOLET, 0.6) })
				.when(enabled, |button| button.hover(|button| button.bg(ca(VIOLET, 0.9))))
				.sm()
				.semibold()
				.text_color(c(ON_VIOLET))
				.child(icon("codicon--debug-start", 16., c(ON_VIOLET)))
				.child(format!("Convert {}", if self.input_paths.is_empty() { String::new() } else { self.file_count() }).trim_end().to_owned())
				.press(self, "convert", Ring::Primary, 6., move |this, _, cx|  {
					if enabled {
						this.convert(cx);
					}
				}, window, cx)
		};
		div().flex().items_center().justify_end().gap(px(12.)).pr(px(10.)).child(div().relative().child(crate::controls::probe("p header")).min_w_0().truncate().xs().text_color(c(DIM)).child(text)).child(action.relative().child(crate::controls::probe("button Convert"))).into_any_element()
	}

	fn notes(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
		let single_webp = (self.inspected.len() == 1).then(|| self.inspected[0].webp).flatten();
		let animated = self.inspected.iter().filter(|entry| entry.animated).count();
		if self.scanned_root.is_none() && animated == 0 && self.drop_rejected == 0 && single_webp.is_none() {
			return None;
		}
		let is_webp = self.settings.as_ref().is_some_and(|settings| settings.format == OutputFormat::Webp);
		let mut notes = div().flex().flex_col().gap(px(4.)).px(px(10.)).xs();
		if self.scanned_root.is_some() {
			let count = self.scanned.len();
			let text = format!(
				"Found {count} image{} in that folder{}{}",
				if count == 1 { "" } else { "s" },
				if self.include_subfolders { ", including subfolders. Symlinks are skipped." } else { ". Subfolders are left out." },
				if self.include_subfolders {
					if self.mirror_structure { " The folder structure is recreated in the destination." } else { " Every file is written side by side in the destination." }
				} else {
					""
				}
			);
			let include = self.include_subfolders;
			let toggle = self.toggle("notes-include", "Include subfolders", None, include, false, |this, on| this.include_subfolders = on, window, cx);
			notes = notes.child(div().flex().flex_wrap().items_start().gap_x(px(24.)).child(div().min_w_0().flex_1().py(px(7.)).text_color(c(DIM)).child(css_text(text))).child(div().flex_none().child(toggle)));
		}
		if let Some(webp) = single_webp {
			let compression = match webp.compression {
				skidbladnir_encode::WebpCompression::Lossy => "lossy",
				skidbladnir_encode::WebpCompression::Lossless => "lossless",
				skidbladnir_encode::WebpCompression::Mixed => "mixed",
			};
			notes = notes.child(div().text_color(c(DIM)).child(css_text(format!("Already a WebP: {}×{}, {compression}{}.", webp.width, webp.height, if webp.has_alpha { ", with alpha" } else { "" }))));
		}
		if animated > 0 {
			let (one, it) = if animated == 1 { ("One queued file is an animation".to_owned(), ("it", "It")) } else { (format!("{animated} queued files are animations"), ("them", "They")) };
			let tail = if is_webp {
				format!("{} will be re-encoded as animated WebP with every frame and its timing kept. Every setting applies to each frame.", it.1)
			} else {
				format!("This format holds still images only here, so {} will be skipped with an error rather than cut down to one frame. Choose WebP to convert {} with every frame.", if animated == 1 { "it" } else { "they" }, it.0)
			};
			notes = notes.child(div().text_color(c(WARNING)).child(css_text(format!("{one} (animated WebP or GIF). {tail}"))));
		}
		if self.drop_rejected > 0 {
			let one = self.drop_rejected == 1;
			notes = notes.child(div().text_color(c(WARNING)).child(css_text(format!("{} dropped {} not a PNG, JPEG, TIFF, WebP, AVIF, JPEG XL, HEIC or GIF and {} skipped.", self.drop_rejected, if one { "file was" } else { "files were" }, if one { "was" } else { "were" }))));
		}
		Some(notes.into_any_element())
	}

	fn progress(&self) -> AnyElement {
		let format = self.settings.as_ref().map_or(OutputFormat::Webp, |settings| settings.format);
		let note = match format {
			OutputFormat::Webp => "libwebp does not always report a final 100%, so the bar can stop short of the end before a file finishes.".to_owned(),
			other => format!("The {} encoder reports no progress while it works, so the bar moves only when a file starts and when it finishes. Cancel cannot stop a file mid-encode, but that file is then discarded rather than written.", format_name(other)),
		};
		let name = self.current_file.as_ref().map_or_else(|| "Starting…".to_owned(), |path| basename(path));
		self.panel(
			"Converting",
			None,
			vec![
				div()
					.px(px(10.))
					.child(div().flex().items_baseline().justify_between().gap(px(12.)).xs().child(div().truncate().text_color(c(BRIGHT)).child(name)).child(div().semibold().text_color(c(ACCENT_TEXT)).child(format!("{} / {}", self.done_count, self.input_paths.len()))))
					.child(div().mt(px(8.)).h(px(8.)).w_full().rounded_full().bg(c(DIM)).overflow_hidden().child(div().h_full().rounded_full().bg(c(ACCENT)).w(gpui::relative(f32::from(u16::try_from(self.current_percent.min(100)).unwrap_or(0)) / 100.))))
					.child(div().mt(px(8.)).xs().text_color(c(DIM)).child(note))
					.into_any_element(),
			],
		)
		.into_any_element()
	}

	fn results(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let dismiss = div()
			.id("dismiss-results")
			.p(px(4.))
			.rounded(px(9.))
			.hover(|button| button.bg(c(FIELD)))
			.press(self, "dismiss-results", Ring::Neutral, 9., |this, _, cx|  {
				this.reports.clear();
				this.failures.clear();
				cx.notify();
			}, window, cx)
			.child(icon("material-symbols--close", 16., c(FG)))
			.into_any_element();
		let mut children = Vec::new();
		if !self.reports.is_empty() {
			let row = |cells: [AnyElement; 5]| {
				let [file, before, after, change, size_] = cells;
				div().flex().items_center().child(div().flex_1().min_w_0().px(px(10.)).py(px(4.)).child(file)).child(div().w(px(90.)).py(px(4.)).flex().justify_end().child(before)).child(div().w(px(90.)).py(px(4.)).flex().justify_end().child(after)).child(div().w(px(80.)).py(px(4.)).flex().justify_end().child(change)).child(div().w(px(110.)).py(px(4.)).pr(px(10.)).flex().justify_end().child(size_))
			};
			let head = |text: &str| div().semibold().child(text.to_owned()).into_any_element();
			let mut table = div().id("results-table").max_h(px(320.)).overflow_y_scroll().xs().child(row([head("File"), head("Before"), head("After"), head("Change"), head("Size")]).text_color(c(DIM)).bg(c(BG)));
			for report in &self.reports {
				let change = match report.saving_percent {
					None => div().child("—").into_any_element(),
					Some(saving) => div().semibold().text_color(c(if saving >= 0.0 { ACCENT_TEXT } else { WARNING })).child(format!("{}{:.1}%", if saving >= 0.0 { "−" } else { "+" }, saving.abs())).into_any_element(),
				};
				table = table.child(row([
					div().text_color(c(BRIGHT)).child(basename(&report.output_path)).into_any_element(),
					div().child(format_bytes(report.conversion.source_bytes)).into_any_element(),
					div().child(format_bytes(report.conversion.output_bytes)).into_any_element(),
					change,
					div().child(format!("{}×{}", report.conversion.width, report.conversion.height)).into_any_element(),
				]));
			}
			children.push(table.into_any_element());
		}
		if !self.failures.is_empty() {
			children.push(div().flex().flex_col().gap(px(4.)).px(px(10.)).xs().text_color(c(ERROR)).children(self.failures.iter().map(|(path, message)| div().child(div().flex().child(div().semibold().text_color(c(BRIGHT)).child(basename(path))).child(format!(": {message}"))))).into_any_element());
		}
		self.panel("Results", Some(dismiss), children).into_any_element()
	}

	fn preview_panel(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let many = self.input_paths.len() > 1;
		let mut row = div().flex().items_center().gap(px(8.)).px(px(10.));
		if many {
			let items: Vec<(usize, String)> = self.input_paths.iter().enumerate().map(|(index, path)| (index, basename(path))).collect();
			let selected = self.preview_path.as_ref().and_then(|current| self.input_paths.iter().position(|path| path == current)).unwrap_or(0);
			let picker = self.select_box("preview-file", &items, selected, false, |this, index| this.preview_path = this.input_paths.get(index).cloned(), window, cx);
			row = row.child(div().w(px(320.)).flex_none().child(picker));
		}
		let label = if self.preview.is_some() { "Preview again" } else { "Preview" };
		let previewing = self.previewing;
		row = row
			.child(
				div()
					.id("preview-run")
					.flex()
					.flex_none()
					.items_center()
					.gap(px(6.))
					.px(px(12.))
					.py(px(7.))
					.rounded(px(6.))
					.bg(c(ACCENT))
					.hover(|button| button.bg(ca(ACCENT, 0.85)))
					.xs()
					.semibold()
					.text_color(c(FG))
					.when(previewing, |button| button.child(icon("lucide--loader-circle", 16., c(FG))))
					.child(label)
					.press(self, "preview-run", Ring::Primary, 6., |this, _, cx|  this.run_preview(cx), window, cx),
			)
			.child(div().min_w_0().flex_1().xs().text_color(c(DIM)).child(css_text(format!("Encodes {} in memory with the current settings. Nothing is written to disk.", if many { "the chosen file" } else { "the file" }))));
		let mut children = vec![row.into_any_element()];
		if !self.preview_error.is_empty() {
			children.push(div().px(px(10.)).xs().text_color(c(ERROR)).child(self.preview_error.clone()).into_any_element());
		}
		if let Some(preview) = &self.preview {
			let figure = |image: Arc<RenderImage>, caption: AnyElement| {
				div()
					.flex()
					.flex_1()
					.min_w_0()
					.flex_col()
					.gap(px(6.))
					.px(px(10.))
					.py(px(7.))
					.child(div().w_full().max_h(px(480.)).aspect_ratio(preview.aspect).rounded(px(12.)).overflow_hidden().relative().child(checkerboard()).child(img(image).absolute().inset_0().size_full().object_fit(ObjectFit::Contain)))
					.child(caption)
			};
			let format = format_name(self.preview_format);
			let saving = preview.saving_percent.map(|saving| div().semibold().text_color(c(if saving >= 0.0 { ACCENT_TEXT } else { WARNING })).child(format!("{}{:.1}%", if saving >= 0.0 { "−" } else { "+" }, saving.abs())));
			let encoded_caption = div()
				.flex()
				.flex_wrap()
				.xs()
				.text_color(c(DIM))
				.child(format!("{} · {} · {}×{}", format.to_uppercase(), format_bytes(preview.encoded_bytes), preview.width, preview.height))
				.when(preview.frames > 1, |caption| caption.child(format!(" · {} frames", preview.frames)))
				.when_some(saving, |caption, saving| caption.child("\u{a0}·\u{a0}").child(saving));
			children.push(
				div()
					.flex()
					.flex_col()
					.gap(px(8.))
					.when(self.preview_stale, |body| body.child(div().px(px(10.)).xs().text_color(c(WARNING)).child("The settings or the file have changed since this preview. Preview again to see them.")))
					.child(div().flex().gap(px(12.)).child(figure(Arc::clone(&preview.original), div().xs().text_color(c(DIM)).child(format!("Original · {}", format_bytes(preview.source_bytes))).into_any_element())).child(figure(Arc::clone(&preview.encoded), encoded_caption.into_any_element())))
					.into_any_element(),
			);
		}
		self.panel("Preview", None, children).into_any_element()
	}

	/// The design's scrollbar: a 6px pill in the rule colour on no track, in an 8px gutter.
	/// Drawn at paint time, from this frame's scroll state rather than the last one's.
	fn scrollbar(&self, cx: &mut Context<Self>) -> AnyElement {
		let handle = self.scroll.clone();
		div()
			.id("scrollbar")
			.relative()
			.w(px(8.))
			.flex_none()
			.h_full()
			.on_mouse_down(
				MouseButton::Left,
				cx.listener(|this, event: &MouseDownEvent, _, cx| {
					let Some((top, length)) = thumb(&this.scroll) else { return };
					let local = event.position.y - this.scroll.bounds().top();
					if local >= top && local <= top + length {
						this.scrollbar_drag = Some(local - top);
					} else {
						this.scroll_to_thumb(local - length / 2.);
					}
					cx.notify();
				}),
			)
			.child(
				canvas(|_, _, _| (), move |bounds, (), window, _| {
					if let Some((top, length)) = thumb(&handle) {
						let pill = Bounds::new(point(bounds.left() + px(1.), bounds.top() + top + px(1.)), size(px(6.), length - px(2.)));
						window.paint_quad(gpui::fill(pill, c(RULE)).corner_radii(px(3.)));
					}
				})
				.absolute()
				.size_full(),
			)
			.into_any_element()
	}

	fn scroll_to_thumb(&mut self, thumb_top: Pixels) {
		let Some((_, length)) = thumb(&self.scroll) else { return };
		let viewport = self.scroll.bounds().size.height;
		let max = self.scroll.max_offset().y;
		let travel = viewport - length;
		if travel > px(0.) {
			let fraction = (thumb_top / travel).clamp(0.0, 1.0);
			self.scroll.set_offset(point(px(0.), -(max * fraction)));
		}
	}
}

impl Render for Skid {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		crate::controls::print_probes();
		if std::env::var_os("SKID_BENCH").is_some()
			&& let Some((what, started)) = self.bench
			&& ((what == "preview" && self.preview.is_some() && !self.previewing) || (what == "convert" && !self.busy && !self.reports.is_empty()))
		{
			self.bench = None;
			window.on_next_frame(move |_, _| eprintln!("bench {what}_ms {}", started.elapsed().as_millis()));
			window.request_animation_frame();
		}
		if std::env::var_os("SKID_PROBE").is_some() {
			eprintln!("viewport {:?} scale {}", window.viewport_size(), window.scale_factor());
		}
		let settings_ready = self.settings.is_some();
		let format = self.settings.as_ref().map(|settings| settings.format);
		let rail = self.rail(window, cx);
		let sidebar = (!self.sidebar_collapsed).then(|| self.sidebar(window, cx));
		let title_bar = self.title_bar(window, cx);
		let header = self.header(window, cx);
		let notes = self.notes(window, cx);
		let mut sections: Vec<AnyElement> = vec![header];
		sections.extend(self.update_banner(window, cx));
		if !self.preferences_notice.is_empty() {
			sections.push(alert(&self.preferences_notice, WARNING, None));
		}
		if !self.startup_error.is_empty() {
			sections.push(alert(&self.startup_error, WARNING, Some("Not connected to the encoder")));
		}
		sections.extend(notes);
		if self.busy {
			sections.push(self.progress());
		}
		if !self.validation_error.is_empty() {
			sections.push(alert(&self.validation_error, ERROR, None));
		}
		if !self.reports.is_empty() || !self.failures.is_empty() {
			sections.push(self.results(window, cx));
		}
		if settings_ready {
			let panel = match format {
				Some(OutputFormat::Avif) => crate::panels::avif::render(self, window, cx),
				Some(OutputFormat::Jxl) => crate::panels::jxl::render(self, window, cx),
				Some(OutputFormat::Heic) => crate::panels::heic::render(self, window, cx),
				_ => crate::panels::webp::render(self, window, cx),
			};
			sections.push(panel);
			sections.push(crate::panels::geometry::render(self, window, cx));
			if !self.input_paths.is_empty() && !self.busy {
				sections.push(self.preview_panel(window, cx));
			}
		}
		let scrollbar = self.scrollbar(cx);

		div()
			.id("window")
			.track_focus(&self.root_focus)
			.size_full()
			.font_family(crate::theme::FONT)
			.regular()
			.xs()
			.text_color(c(FG))
			.on_action(cx.listener(|_, _: &crate::FocusNext, window, cx| window.focus_next(cx)))
			.on_action(cx.listener(|_, _: &crate::FocusPrevious, window, cx| window.focus_prev(cx)))
			.on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
				if this.drag.is_some() && event.pressed_button == Some(MouseButton::Left) {
					this.drag_slider(event.position.x, cx);
				} else if let Some(grab) = this.scrollbar_drag
					&& event.pressed_button == Some(MouseButton::Left)
				{
					let local = event.position.y - this.scroll.bounds().top();
					this.scroll_to_thumb(local - grab);
					cx.notify();
				}
			}))
			.on_mouse_up(
				MouseButton::Left,
				cx.listener(|this, _: &MouseUpEvent, _, cx| {
					this.drag = None;
					this.scrollbar_drag = None;
					cx.notify();
				}),
			)
			.on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
				let menu_open = matches!(this.popup, Some(Popup::QueueMenu | Popup::Select(_)));
				match event.keystroke.key.as_str() {
					"escape" if this.popup.is_some() => {
						this.popup = None;
						this.menu_highlight = None;
					}
					"down" | "up" | "home" | "end" if menu_open => {
						let last = this.menu_actions.len().saturating_sub(1);
						this.menu_highlight = Some(match (event.keystroke.key.as_str(), this.menu_highlight) {
							("down", Some(index)) => (index + 1).min(last),
							("up", Some(index)) => index.saturating_sub(1),
							("home" | "down", _) => 0,
							_ => last,
						});
					}
					"enter" | "space" if menu_open => {
						if let Some(action) = this.menu_highlight.and_then(|index| this.menu_actions.get(index)).cloned() {
							this.swallow_click = true;
							action(this, window, cx);
						}
					}
					_ => return,
				}
				cx.stop_propagation();
				cx.notify();
			}))
			.on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.dropped(paths, cx)))
			.child(
				// The window's own frame: the window is frameless and transparent, so this
				// rounded surface is the whole visible window.
				div()
					.relative()
					.child(crate::controls::probe("frame"))
					.flex()
					.size_full()
					.overflow_hidden()
					.rounded(px(32.))
					.bg(c(BG))
					.child(rail)
					.children(sidebar)
					.child(
						div()
							.relative()
							.child(crate::controls::probe("main"))
							.flex()
							.min_w_0()
							.flex_1()
							.flex_col()
							.gap(px(16.))
							.pb(px(6.))
							.pl(px(if self.sidebar_collapsed { 8. } else { 24. }))
							.child(title_bar)
							.child(div().flex().min_h_0().flex_1().child(div().id("scroll").flex_1().min_w_0().overflow_y_scroll().track_scroll(&self.scroll).pr(px(16.)).pb(px(24.)).child(div().w_full().flex().flex_col().gap(px(16.)).children(sections))).child(scrollbar)),
					)
					.child(
						// The drop target's outline, while files are dragged over the window.
						div()
							.absolute()
							.top(px(16.))
							.left(px(16.))
							.right(px(16.))
							.bottom(px(16.))
							.rounded(px(28.))
							.invisible()
							.drag_over::<ExternalPaths>(|overlay, _, _, _| overlay.visible().flex().items_center().justify_center().bg(ca(BG, 0.9)).border_2().border_dashed().border_color(c(ACCENT)).sm().semibold().text_color(c(ACCENT_TEXT)))
							.child("Drop images to convert"),
					),
			)
	}
}

/// The thumb's top and length in the scroll viewport, when there is anything to scroll.
fn thumb(scroll: &ScrollHandle) -> Option<(Pixels, Pixels)> {
	let viewport = scroll.bounds().size.height;
	let max = scroll.max_offset().y;
	if max <= px(0.) || viewport <= px(0.) {
		return None;
	}
	let length = (viewport * (viewport / (viewport + max))).max(px(24.));
	Some(((viewport - length) * (-scroll.offset().y / max), length))
}

/// `data-tauri-drag-region`: press to move the window, double-press to maximise it.
fn drag_window(event: &MouseDownEvent, window: &mut Window) {
	if event.click_count == 2 {
		window.zoom_window();
	} else {
		window.start_window_move();
	}
}

/// The "Collapse" and "Expand" links: `UButton variant="link"` with a turned chevron.
fn link_button(id: &str, label: &str, degrees: f32) -> gpui::Stateful<gpui::Div> {
	div()
		.id(SharedString::from(id.to_owned()))
		.flex()
		.flex_none()
		.items_center()
		.gap(px(6.))
		.px(px(4.))
		.py(px(6.))
		.rounded(px(9.))
		.xs()
		.semibold()
		.text_color(c(FG))
		.child(label.to_owned())
		.child(icon("subway--down-2", 16., c(FG)).with_transformation(gpui::Transformation::rotate(gpui::radians(degrees.to_radians()))))
}

/// The Queue and Destination buttons: a title line and a subtitle.
fn sidebar_button(id: &str, icon_name: &str, title: &str, subtitle: &str, badge: Option<String>) -> gpui::Stateful<gpui::Div> {
	div()
		.id(SharedString::from(id.to_owned()))
		.relative()
		.child(crate::controls::probe(&format!("button {id}")))
		.flex()
		.w_full()
		.flex_col()
		.gap(px(4.))
		.rounded(px(12.))
		.px(px(10.))
		.py(px(7.))
		.hover(|button| button.bg(ca(FIELD, 0.7)))
		.child(
			div()
				.flex()
				.w_full()
				.items_center()
				.gap(px(8.))
				.child(icon(icon_name, 16., c(FG)))
				.child(div().relative().child(crate::controls::probe(&format!("span {title}"))).min_w_0().flex_1().xs().text_color(c(BRIGHT)).child(title.to_owned()))
				.when_some(badge, |line, badge| line.child(div().rounded_full().bg(c(ACCENT)).px(px(6.)).py(px(1.)).text_size(px(11.)).line_height(px(16.)).semibold().text_color(c(FG)).child(badge))),
		)
		.child(div().relative().child(crate::controls::probe(&format!("span {subtitle}"))).w_full().truncate().xs().text_color(c(DIM)).child(subtitle.to_owned()))
}

fn primary_button(id: &str, label: &str, disabled: bool) -> gpui::Stateful<gpui::Div> {
	div()
		.id(SharedString::from(id.to_owned()))
		.flex()
		.flex_none()
		.items_center()
		.px(px(10.))
		.py(px(6.))
		.rounded(px(9.))
		.bg(c(ACCENT))
		.when(disabled, |button| button.opacity(0.75))
		.when(!disabled, |button| button.hover(|button| button.bg(ca(ACCENT, 0.75))))
		.xs()
		.medium()
		.text_color(c(BG))
		.child(label.to_owned())
}

/// Nuxt UI's soft `UAlert`.
fn alert(text: &str, colour: u32, title: Option<&str>) -> AnyElement {
	div()
		.flex()
		.flex_col()
		.gap(px(4.))
		.p(px(16.))
		.rounded(px(12.))
		.bg(ca(colour, 0.1))
		.text_color(c(colour))
		.when_some(title, |alert, title| alert.child(div().sm().medium().child(css_text(title.to_owned()))))
		.child(div().sm().opacity(0.9).child(css_text(text.to_owned())))
		.into_any_element()
}

/// The `checkerboard` utility: 8px squares of the rule colour on the field colour.
fn checkerboard() -> impl IntoElement {
	canvas(
		|_, _, _| (),
		|bounds, (), window, _| {
			window.paint_quad(gpui::fill(bounds, c(FIELD)));
			let square = px(8.);
			let columns = (bounds.size.width / square).ceil() as i32;
			let rows = (bounds.size.height / square).ceil() as i32;
			for row in 0..rows {
				for column in 0..columns {
					if (row + column) % 2 == 0 {
						let origin = point(bounds.left() + square * column as f32, bounds.top() + square * row as f32);
						window.paint_quad(gpui::fill(Bounds::new(origin, size(square, square)).intersect(&bounds), c(RULE)));
					}
				}
			}
		},
	)
	.absolute()
	.size_full()
}

/// A WebP decoded to the GPU's BGRA, every frame of an animation with its delay. A still
/// image is decoded by libwebp, as WebKitGTK decodes it; an animation by image-rs.
fn texture(webp: &[u8]) -> Result<Arc<RenderImage>, String> {
	use image::AnimationDecoder as _;
	if let Ok(still) = source::decode(Path::new("preview.webp"), webp.to_vec()) {
		let mut pixels = still.pixels;
		for pixel in pixels.as_chunks_mut::<4>().0 {
			pixel.swap(0, 2);
		}
		let buffer = image::RgbaImage::from_raw(still.width, still.height, pixels).ok_or("the decoded buffer does not match its size")?;
		return Ok(Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])));
	}
	let decoder = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(webp)).map_err(|error| error.to_string())?;
	let mut frames: smallvec::SmallVec<[image::Frame; 1]> = smallvec::SmallVec::new();
	for frame in decoder.into_frames() {
		let frame = frame.map_err(|error| error.to_string())?;
		let delay = frame.delay();
		let mut buffer = frame.into_buffer();
		for pixel in buffer.as_flat_samples_mut().samples.as_chunks_mut::<4>().0 {
			pixel.swap(0, 2);
		}
		frames.push(image::Frame::from_parts(buffer, 0, 0, delay));
	}
	Ok(Arc::new(RenderImage::new(frames)))
}

fn image_aspect(webp: &[u8]) -> Option<f32> {
	let (width, height) = skidbladnir_encode::inspect_webp(webp).map(|info| (info.width, info.height))?;
	#[allow(clippy::cast_precision_loss)]
	Some(width as f32 / height.max(1) as f32)
}

pub fn basename(path: &Path) -> String {
	path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

pub fn format_name(format: OutputFormat) -> &'static str {
	match format {
		OutputFormat::Webp => "WebP",
		OutputFormat::Avif => "AVIF",
		OutputFormat::Jxl => "JPEG XL",
		OutputFormat::Heic => "HEIC",
	}
}

/// `formatBytes` from `useSettings.ts`.
pub fn format_bytes(bytes: u64) -> String {
	#[allow(clippy::cast_precision_loss)]
	let value = bytes as f64;
	if bytes < 1024 {
		format!("{bytes} B")
	} else if value < 1024.0 * 1024.0 {
		format!("{:.1} KB", value / 1024.0)
	} else {
		format!("{:.2} MB", value / 1024.0 / 1024.0)
	}
}
