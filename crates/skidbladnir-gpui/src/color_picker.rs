//! The blend colour's picker.
//!
//! The old window's `<input type="color">` opens whatever its engine provides: GTK's colour
//! chooser under `WebKitGTK`, Chromium's popup under `WebView2`, the colour panel on macOS. gpui
//! has none, so this is one picker for every platform, in the window's own pop-up style: a
//! saturation and brightness square, a hue strip, and a `#rrggbb` field.
//!
//! From the keyboard, the square and the strip are sliders: the arrows move them by 1%
//! (shift: 10%), Home and End go to either end. Opening the picker focuses the square.

use std::rc::Rc;

use gpui::{AnyElement, AppContext, Context, ElementId, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Window, div, hsla, linear_color_stop, linear_gradient, prelude::FluentBuilder, px, rgb};
use gpuikit::{elements::input::input, input::InputState};

use crate::{
	app::{Popup, Skid}, controls::{Press, Ring, Set, field, field_label, place_local, popup_panel}, theme::{ACCENT, BRIGHT, FG, RULE, Type, c, ca}
};

const SQUARE_WIDTH: f32 = 224.;
const SQUARE_HEIGHT: f32 = 144.;

/// Which part of the picker the pointer is dragging.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Part {
	Square,
	Hue,
}

pub struct ColorDrag {
	pub part: Part,
	pub key: SharedString,
	pub set: Set<u32>,
}

/// `0xRRGGBB` to hue (0..1), saturation and value.
#[must_use]
#[expect(clippy::float_cmp, reason = "`max` is one of the three channels, so equality is exact; it picks the hue sector")]
pub fn to_hsv(colour: u32) -> (f32, f32, f32) {
	#[allow(clippy::cast_precision_loss)]
	let [r, g, b] = [(colour >> 16) & 0xff, (colour >> 8) & 0xff, colour & 0xff].map(|channel| channel as f32 / 255.);
	let max = r.max(g).max(b);
	let min = r.min(g).min(b);
	let delta = max - min;
	let hue = if delta == 0. {
		0.
	} else if max == r {
		((g - b) / delta).rem_euclid(6.) / 6.
	} else if max == g {
		((b - r) / delta + 2.) / 6.
	} else {
		((r - g) / delta + 4.) / 6.
	};
	(hue, if max == 0. { 0. } else { delta / max }, max)
}

/// Hue (0..1), saturation and value to `0xRRGGBB`.
#[must_use]
pub fn from_hsv(hue: f32, saturation: f32, value: f32) -> u32 {
	let sector = (hue.rem_euclid(1.) * 6.).min(5.999_99);
	let chroma = value * saturation;
	let x = chroma * (1. - ((sector % 2.) - 1.).abs());
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	let (r, g, b) = match sector as u32 {
		0 => (chroma, x, 0.),
		1 => (x, chroma, 0.),
		2 => (0., chroma, x),
		3 => (0., x, chroma),
		4 => (x, 0., chroma),
		_ => (chroma, 0., x),
	};
	let offset = value - chroma;
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	let byte = |channel: f32| ((channel + offset) * 255.).round().clamp(0., 255.) as u32;
	(byte(r) << 16) | (byte(g) << 8) | byte(b)
}

/// `#rrggbb`, or `rrggbb`, to a colour.
#[must_use]
pub fn parse_hex(text: &str) -> Option<u32> {
	let hex = text.trim().trim_start_matches('#');
	(hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}

impl Skid {
	/// `ControlColor`: a swatch that opens the picker, and the `0xRRGGBB` value.
	pub fn color(&mut self, id: &str, label: &str, value: u32, set: impl Fn(&mut Self, u32) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let key: SharedString = format!("color-{id}").into();
		let open = self.popup == Some(Popup::Color(key.clone()));
		let set: Set<u32> = Rc::new(set);
		let open_key = key.clone();
		let panel = open.then(|| self.color_panel(&key, value, &set, window, cx));
		let swatch = div().id(ElementId::Name(key.clone())).children(panel).child(self.record(&key)).size(px(28.)).flex_none().rounded(px(6.)).p(px(4.)).child(div().size_full().rounded(px(2.)).bg(rgb(value)).border_1().border_color(c(RULE))).press(
			self,
			&key,
			label,
			Ring::Neutral,
			6.,
			move |this, window, cx| {
				this.popup = if this.popup == Some(Popup::Color(open_key.clone())) { None } else { Some(Popup::Color(open_key.clone())) };
				this.picker_hsv = Some(to_hsv(value));
				if this.popup.is_some() {
					let square = this.focus_handle_for(&format!("{open_key}-square"), cx);
					window.focus(&square, cx);
				}
				cx.notify();
			},
			window,
			cx,
		);

		field().child(field_label(label.to_owned())).child(div().flex().items_center().gap(px(8.)).child(swatch).child(div().text_color(c(BRIGHT)).child(format!("0x{value:06x}")))).into_any_element()
	}

	#[allow(clippy::too_many_lines, reason = "one pop-up's element tree: the square, the strip and the field, with their keys")]
	fn color_panel(&mut self, key: &SharedString, value: u32, set: &Set<u32>, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let (hue, saturation, brightness) = self.picker_hsv.unwrap_or_else(|| to_hsv(value));
		let square_key: SharedString = format!("{key}-square").into();
		let hue_key: SharedString = format!("{key}-hue").into();
		let square_focus = self.focus_handle_for(&square_key, cx);
		let hue_focus = self.focus_handle_for(&hue_key, cx);
		let pure = hsla(hue, 1., 0.5, 1.);
		let clear = |colour: Hsla| Hsla { a: 0., ..colour };
		let white = hsla(0., 0., 1., 1.);
		let black = hsla(0., 0., 0., 1.);

		let start = |part: Part| {
			let set = Rc::clone(set);
			let key = key.clone();
			cx.listener(move |this: &mut Self, event: &MouseDownEvent, _, cx| {
				this.color_drag = Some(ColorDrag { part, key: key.clone(), set: Rc::clone(&set) });
				this.drag_color(event.position, cx);
			})
		};
		// The arrow keys, as on a slider: `step` turns a key into the new value of one channel.
		let keys = |part: Part| {
			let set = Rc::clone(set);
			cx.listener(move |this: &mut Self, event: &KeyDownEvent, _, cx| {
				let (hue, saturation, brightness) = this.picker_hsv.unwrap_or((0., 0., 1.));
				let step = if event.keystroke.modifiers.shift { 0.1 } else { 0.01 };
				let key = event.keystroke.key.as_str();
				let next = match (part, key) {
					(Part::Square, "left") => (hue, (saturation - step).max(0.), brightness),
					(Part::Square, "right") => (hue, (saturation + step).min(1.), brightness),
					(Part::Square, "down") => (hue, saturation, (brightness - step).max(0.)),
					(Part::Square, "up") => (hue, saturation, (brightness + step).min(1.)),
					(Part::Square, "home") => (hue, 0., brightness),
					(Part::Square, "end") => (hue, 1., brightness),
					(Part::Hue, "left" | "down") => ((hue - step).max(0.), saturation, brightness),
					(Part::Hue, "right" | "up") => ((hue + step).min(0.999), saturation, brightness),
					(Part::Hue, "home") => (0., saturation, brightness),
					(Part::Hue, "end") => (0.999, saturation, brightness),
					_ => return,
				};
				this.picker_hsv = Some(next);
				set(this, from_hsv(next.0, next.1, next.2));
				this.changed(cx);
				cx.stop_propagation();
			})
		};
		let square_ring = Self::ring_visible(&square_focus, window);
		let hue_ring = Self::ring_visible(&hue_focus, window);
		#[allow(clippy::cast_possible_truncation)]
		let percent = |fraction: f32| (fraction * 100.).round() as i32;
		let square = div().id(ElementId::Name(square_key.clone())).role(gpui::Role::Slider).aria_label("Saturation and brightness").aria_description(SharedString::from(format!("Saturation {}%, brightness {}%", percent(saturation), percent(brightness)))).aria_numeric_value(f64::from(percent(saturation))).track_focus(&square_focus).on_key_down(keys(Part::Square)).relative().w(px(SQUARE_WIDTH)).h(px(SQUARE_HEIGHT)).rounded(px(6.)).overflow_hidden().bg(pure).child(self.record(&square_key)).child(div().absolute().inset_0().bg(linear_gradient(90., linear_color_stop(white, 0.), linear_color_stop(clear(white), 1.)))).child(div().absolute().inset_0().bg(linear_gradient(180., linear_color_stop(clear(black), 0.), linear_color_stop(black, 1.)))).child(div().absolute().left(px(saturation * SQUARE_WIDTH - 6.)).top(px((1. - brightness) * SQUARE_HEIGHT - 6.)).size(px(12.)).rounded_full().border_2().border_color(white).shadow_sm()).on_mouse_down(MouseButton::Left, start(Part::Square));
		let square = div().relative().child(square).when(square_ring, |wrap| wrap.child(crate::controls::ring(3., ca(ACCENT, 0.25), 0., 6.)));
		// Six segments, red to red, each a two-stop gradient.
		let segments = (0..6).map(|segment| {
			#[allow(clippy::cast_precision_loss)]
			let (from, to) = (segment as f32 / 6., (segment + 1) as f32 / 6.);
			div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(hsla(from, 1., 0.5, 1.), 0.), linear_color_stop(hsla(to, 1., 0.5, 1.), 1.)))
		});
		let strip = div().id(ElementId::Name(hue_key.clone())).role(gpui::Role::Slider).aria_label("Hue").aria_numeric_value(f64::from((hue * 360.).round())).track_focus(&hue_focus).on_key_down(keys(Part::Hue)).when(hue_ring, |strip| strip.child(crate::controls::ring(3., ca(ACCENT, 0.25), 0., 6.))).relative().w(px(SQUARE_WIDTH)).h(px(12.)).child(self.record(&hue_key)).child(div().size_full().flex().rounded_full().overflow_hidden().children(segments)).child(div().absolute().top(px(-2.)).left(px(hue * SQUARE_WIDTH - 8.)).size(px(16.)).rounded_full().border_2().border_color(white).bg(pure).shadow_sm()).on_mouse_down(MouseButton::Left, start(Part::Hue));

		let hex = self.hex_input(key, value, Rc::clone(set), cx);
		let anchor = self.bounds_of(key).unwrap_or_default();
		if std::env::var_os("SKID_PROBE").is_some() {
			eprintln!("picker anchor {anchor:?}");
		}
		let body = div().flex().flex_col().gap(px(10.)).p(px(12.)).child(square).child(strip).child(div().flex().items_center().gap(px(8.)).child(div().size(px(28.)).flex_none().rounded(px(6.)).bg(rgb(value)).border_1().border_color(c(RULE))).child(input(&hex, cx).id(format!("{key}-hex-field")).aria_label("Hex colour").flex_1().h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE)).xs().text_color(c(FG))));
		place_local(gpui::Anchor::TopLeft, gpui::point(px(0.), anchor.size.height + px(8.)), popup_panel(&format!("{key}-panel"), None, body, cx).role(gpui::Role::Dialog).aria_label("Choose a colour")).into_any_element()
	}

	/// The `#rrggbb` field, kept in step with the colour unless it is being typed in.
	fn hex_input(&mut self, key: &SharedString, value: u32, set: Set<u32>, cx: &mut Context<Self>) -> gpui::Entity<InputState> {
		let input_key: SharedString = format!("{key}-hex").into();
		let text = format!("#{value:06x}");
		if let Some(state) = self.inputs.get(&input_key).cloned() {
			if self.editing.as_ref() != Some(&input_key) && state.read(cx).content() != text {
				state.update(cx, |state, cx| state.set_content_silent(text, cx));
			}
			return state;
		}
		let state = cx.new(|cx| {
			let mut state = InputState::new_singleline(cx);
			state.set_content_silent(text, cx);
			state
		});
		let editing = input_key.clone();
		self.subscriptions.push(cx.subscribe(&state, move |this, state, event: &gpuikit::input::InputStateEvent, cx| match event {
			gpuikit::input::InputStateEvent::Focus => this.editing = Some(editing.clone()),
			gpuikit::input::InputStateEvent::Blur => this.editing = None,
			gpuikit::input::InputStateEvent::TextChanged => {
				if let Some(colour) = parse_hex(state.read(cx).content()) {
					this.picker_hsv = Some(to_hsv(colour));
					set(this, colour);
					this.changed(cx);
				}
			}
			_ => {}
		}));
		self.inputs.insert(input_key, state.clone());
		state
	}

	/// Follow the pointer in the square or the hue strip.
	pub fn drag_color(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
		let Some(drag) = &self.color_drag else { return };
		let (part, set) = (drag.part, Rc::clone(&drag.set));
		let bounds_key = format!("{}-{}", drag.key, if part == Part::Square { "square" } else { "hue" });
		let Some(bounds) = self.bounds_of(&bounds_key) else { return };
		let x = ((position.x - bounds.left()) / bounds.size.width).clamp(0., 1.);
		let y = ((position.y - bounds.top()) / bounds.size.height).clamp(0., 1.);
		let (hue, saturation, value) = self.picker_hsv.unwrap_or((0., 0., 1.));
		let next = if part == Part::Square { (hue, x, 1. - y) } else { (x.min(0.999), saturation, value) };
		self.picker_hsv = Some(next);
		set(self, from_hsv(next.0, next.1, next.2));
		self.changed(cx);
	}
}

#[cfg(test)]
mod tests {
	use super::{from_hsv, parse_hex, to_hsv};

	#[test]
	fn hsv_round_trips() {
		for colour in [0x00ff_ffff, 0x0000_0000, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0x007f_22fe, 0x00d8_d8d0, 0x005e_a500] {
			let (h, s, v) = to_hsv(colour);
			assert_eq!(from_hsv(h, s, v), colour, "{colour:06x}");
		}
	}

	#[test]
	fn hex_parses() {
		assert_eq!(parse_hex("#5ea500"), Some(0x005e_a500));
		assert_eq!(parse_hex("ffffff"), Some(0x00ff_ffff));
		assert_eq!(parse_hex("#fff"), None);
	}
}
