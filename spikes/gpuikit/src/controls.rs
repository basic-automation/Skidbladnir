//! The old window's controls (`frontend/components/Control*.vue`), rebuilt to the geometry
//! the 1.0.0 frontend computes. Each is a method on [`Skid`] that renders from the current
//! settings and writes back through a setter, the way `v-model` does: there is one copy of
//! every value, the settings struct.

use std::rc::Rc;

use gpui::{
	AnyElement, AppContext, Bounds, ClickEvent, Context, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder, px, relative, svg
};
use gpuikit::{elements::input::input, input::InputState};

use crate::{
	app::{Popup, Skid}, theme::{ACCENT, ACCENT_TEXT, BG, BRIGHT, DIM, FG, FIELD, MUTED, RULE, Type, VIOLET, c, ca}
};

/// Writes a new value into the window's state.
pub type Set<T> = Rc<dyn Fn(&mut Skid, T)>;

/// A slider being dragged: the pointer's x maps onto its track.
pub struct SliderDrag {
	pub id: SharedString,
	pub min: f64,
	pub max: f64,
	pub step: f64,
	pub set: Set<f64>,
}

/// The ring `:focus-visible` draws: 2px of violet, 2px outside the element.
pub fn focus_ring(radius: f32) -> gpui::Div {
	div().absolute().top(px(-4.)).left(px(-4.)).right(px(-4.)).bottom(px(-4.)).border_2().border_color(c(VIOLET)).rounded(px(radius + 4.))
}

/// One labelled control's frame: `flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs`.
pub fn field() -> gpui::Div {
	div().flex().flex_col().min_w_0().gap(px(8.)).px(px(10.)).py(px(7.)).xs()
}

pub fn field_label(label: impl Into<SharedString>) -> gpui::Div {
	div().xs().semibold().text_color(c(FG)).child(label.into())
}

pub fn help(text: impl Into<SharedString>) -> gpui::Div {
	div().xs().regular().text_color(c(DIM)).child(text.into())
}

/// An Iconify icon, tinted like `currentColor`.
pub fn icon(name: &str, size: f32, colour: Hsla) -> gpui::Svg {
	svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).flex_none().text_color(colour)
}

impl Skid {
	/// The focus handle for a control, made on first use so the tab order follows layout.
	pub fn focus_handle_for(&mut self, id: &str, cx: &mut Context<Self>) -> FocusHandle {
		self.focus.entry(SharedString::from(id.to_owned())).or_insert_with(|| cx.focus_handle().tab_stop(true)).clone()
	}

	/// Whether this control shows its focus ring.
	pub fn ring_visible(handle: &FocusHandle, window: &Window) -> bool {
		handle.is_focused(window) && window.last_input_was_keyboard()
	}

	/// `ControlPanel`: a heading over its controls, with no frame.
	pub fn panel(&self, title: &str, actions: Option<AnyElement>, children: Vec<AnyElement>) -> gpui::Div {
		div()
			.flex()
			.flex_col()
			.w_full()
			.gap(px(16.))
			.child(div().flex().items_center().gap(px(12.)).child(div().relative().child(probe(&format!("h2 {title}"))).min_w_0().flex_1().sm().semibold().text_color(c(BRIGHT)).child(SharedString::from(title.to_owned()))).children(actions))
			.children(children)
	}

	/// `grid grid-cols-N gap-G`: equal columns, each child in its own cell.
	pub fn grid(columns: usize, gap: f32, children: Vec<AnyElement>) -> gpui::Div {
		let mut cells: Vec<AnyElement> = children.into_iter().map(|child| div().flex_1().min_w_0().flex().flex_col().child(child).into_any_element()).collect();
		while cells.len() % columns != 0 {
			cells.push(div().flex_1().min_w_0().into_any_element());
		}
		let mut rows = div().flex().flex_col().gap(px(gap));
		let mut cells = cells.into_iter().peekable();
		while cells.peek().is_some() {
			rows = rows.child(div().flex().gap(px(gap)).children(cells.by_ref().take(columns)));
		}
		rows
	}

	/// `ControlToggle`: Nuxt UI's checkbox in the Nanna skin — a filled square, muted when
	/// off and accent when on, with the label and the help beside it.
	#[allow(clippy::too_many_arguments)]
	pub fn toggle(&mut self, id: &str, label: &str, help_text: Option<&str>, checked: bool, disabled: bool, set: impl Fn(&mut Self, bool) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let focus = self.focus_handle_for(id, cx);
		let ring = Self::ring_visible(&focus, window);
		let set = Rc::new(set);
		let toggle = {
			let set = Rc::clone(&set);
			move |this: &mut Self, cx: &mut Context<Self>| {
				if !disabled {
					set(this, !checked);
					this.changed(cx);
				}
			}
		};
		let toggle = Rc::new(toggle);
		let on_key = Rc::clone(&toggle);
		div()
			.id(ElementId::Name(SharedString::from(format!("toggle-{id}"))))
			.flex()
			.items_start()
			.min_w_0()
			.px(px(10.))
			.py(px(7.))
			.when(!disabled, |row| row.cursor_pointer())
			.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, cx)))
			.child(
				div().h(px(20.)).flex().items_center().mt(px(-2.)).child(
					div()
						.relative()
						.size(px(16.))
						.rounded(px(4.))
						.bg(c(if checked { ACCENT } else { MUTED }))
						.when(disabled, |box_| box_.opacity(0.75))
						.flex()
						.items_center()
						.justify_center()
						.track_focus(&focus)
						.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
							if event.keystroke.key == "space" {
								on_key(this, cx);
								cx.stop_propagation();
							}
						}))
						.when(checked, |box_| box_.child(icon("lucide--check", 14., c(BG))))
						.when(ring, |box_| box_.child(focus_ring(4.))),
				),
			)
			.child(
				div()
					.ml(px(8.))
					.flex()
					.flex_col()
					.gap(px(4.))
					.min_w_0()
					.child(div().relative().child(probe(&format!("label {label}"))).xs().semibold().text_color(c(FG)).when(disabled, |label| label.opacity(0.75)).child(SharedString::from(label.to_owned())))
					.when_some(help_text, |wrapper, text| wrapper.child(help(text.to_owned()).relative().child(probe(&format!("p {}", &text[..text.len().min(24)]))))),
			)
			.into_any_element()
	}

	/// `ControlSlider`: a label with a live readout, Reka's slider, and the help. With
	/// `decimals`, the readout is a field, so a fractional value can be typed exactly.
	#[allow(clippy::too_many_arguments)]
	pub fn slider(&mut self, id: &str, label: &str, value: f64, min: f64, max: f64, step: f64, decimals: Option<usize>, help_text: Option<&str>, disabled: bool, divisor: Option<f64>, set: impl Fn(&mut Self, f64) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let id: SharedString = id.to_owned().into();
		let set: Set<f64> = Rc::new(set);
		let focus = self.focus_handle_for(&id, cx);
		let ring = Self::ring_visible(&focus, window);
		let fraction = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };

		let readout: AnyElement = match decimals {
			Some(places) => {
				let state = self.number_input(&id, value, places, min, max, Rc::clone(&set), cx);
				// UInputNumber here is `w-16 text-right`. gpuikit's input always draws its text
				// from the left, so the field is sized to its own text inside a right-aligned
				// 64px slot, which puts the digits where the old window puts them.
				let width = text_width(state.read(cx).content(), crate::theme::SEMIBOLD, window);
				div().w(px(64.)).flex().justify_end().child(input(&state, cx).w(width).h(px(16.)).xs().semibold().text_color(c(ACCENT_TEXT)).bg(gpui::transparent_black())).into_any_element()
			}
			None => {
				let text = divisor.map_or_else(|| format_number(value, 0), |divisor| format!("{:.1}", value / divisor));
				div().text_color(c(if disabled { DIM } else { ACCENT_TEXT })).child(text).into_any_element()
			}
		};

		let bounds_store = Rc::clone(&self.slider_bounds);
		let store_id = id.clone();
		let drag_set = Rc::clone(&set);
		let drag_id = id.clone();
		let key_set = Rc::clone(&set);
		let thumb = 14.0_f32;

		field()
			.child(
				div()
					.flex()
					.items_baseline()
					.gap(px(12.))
					.xs()
					.semibold()
					.child(div().min_w_0().flex_1().text_color(c(FG)).child(SharedString::from(label.to_owned())).when(disabled, |label| label.child(div().regular().text_color(c(DIM)).child(" · not in use"))))
					.child(readout),
			)
			.child(
				div()
					.id(ElementId::Name(format!("slider-{id}").into()))
					.relative()
					.h(px(7.))
					.w_full()
					.when(disabled, |track| track.opacity(0.5))
					.child(canvas(|bounds, _, _| bounds, move |_, bounds: Bounds<Pixels>, _, _| {
						bounds_store.borrow_mut().insert(store_id.clone(), bounds);
					}).absolute().size_full())
					.child(div().absolute().inset_0().rounded_full().bg(c(DIM)))
					.child(div().absolute().top_0().bottom_0().left_0().w(relative(fraction as f32)).rounded_full().bg(c(ACCENT)))
					.child(
						// Reka keeps the thumb inside the track: its centre moves from half a thumb
						// in at the minimum to half a thumb in at the maximum.
						div()
							.absolute()
							.top(px(3.5 - thumb / 2. - 2.))
							.left(relative(fraction as f32))
							.ml(px((0.5 - fraction as f32) * thumb - thumb / 2. - 2.))
							.size(px(thumb + 4.))
							.rounded_full()
							.border_2()
							.border_color(c(ACCENT))
							.bg(c(BG))
							.track_focus(&focus)
							.when(ring, |thumb_| thumb_.child(focus_ring(9.)))
							.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
								let delta = match event.keystroke.key.as_str() {
									"right" | "up" => step,
									"left" | "down" => -step,
									"pageup" => step * 10.0,
									"pagedown" => -step * 10.0,
									"home" => min - max,
									"end" => max - min,
									_ => return,
								};
								if !disabled {
									key_set(this, snap(value + delta, min, max, step));
									this.changed(cx);
								}
								cx.stop_propagation();
							})),
					)
					.when(!disabled, |track| {
						track.on_mouse_down(
							MouseButton::Left,
							cx.listener(move |this, event: &MouseDownEvent, window, cx| {
								this.drag = Some(SliderDrag { id: drag_id.clone(), min, max, step, set: Rc::clone(&drag_set) });
								this.drag_slider(event.position.x, cx);
								window.focus(&this.focus_handle_for(&drag_id, cx), cx);
							}),
						)
					}),
			)
			.when_some(help_text, |frame, text| frame.child(help(text.to_owned())))
			.into_any_element()
	}

	/// Move the dragged slider to the pointer.
	pub fn drag_slider(&mut self, x: Pixels, cx: &mut Context<Self>) {
		let Some(drag) = &self.drag else { return };
		let Some(bounds) = self.slider_bounds.borrow().get(&drag.id).copied() else { return };
		let fraction = f64::from(((x - bounds.left()) / bounds.size.width).clamp(0.0, 1.0));
		let value = snap(drag.min + fraction * (drag.max - drag.min), drag.min, drag.max, drag.step);
		let set = Rc::clone(&drag.set);
		set(self, value);
		self.changed(cx);
	}

	/// The text state behind a number field, kept in step with the value it edits.
	#[allow(clippy::too_many_arguments)]
	pub fn number_input(&mut self, id: &str, value: f64, decimals: usize, min: f64, max: f64, set: Set<f64>, cx: &mut Context<Self>) -> gpui::Entity<InputState> {
		let key: SharedString = id.to_owned().into();
		let text = format_number(value, decimals);
		if let Some(state) = self.inputs.get(&key) {
			let state = state.clone();
			// Show the model's value unless the user is typing in this field.
			if !self.editing.as_ref().is_some_and(|editing| *editing == key) && state.read(cx).content() != text {
				state.update(cx, |state, cx| state.set_content_silent(text, cx));
			}
			return state;
		}
		let state = cx.new(|cx| {
			let mut state = InputState::new_singleline(cx);
			state.set_content_silent(text, cx);
			state
		});
		let editing_key = key.clone();
		self.subscriptions.push(cx.subscribe(&state, move |this, state, event: &gpuikit::input::InputStateEvent, cx| match event {
			gpuikit::input::InputStateEvent::Focus => this.editing = Some(editing_key.clone()),
			gpuikit::input::InputStateEvent::Blur => {
				this.editing = None;
				cx.notify();
			}
			gpuikit::input::InputStateEvent::TextChanged => {
				// As UInputNumber: a value that parses is clamped and taken; one that does not
				// (an empty field mid-edit) leaves the model as it was.
				if let Ok(parsed) = state.read(cx).content().trim().parse::<f64>()
					&& parsed.is_finite()
				{
					set(this, parsed.clamp(min, max));
					this.changed(cx);
				}
			}
			_ => {}
		}));
		self.inputs.insert(key, state.clone());
		state
	}

	/// `ChoiceGroup` under a heading (`ControlChoice`): the chosen item carries an accent bar
	/// on its left edge and an accent label.
	#[allow(clippy::too_many_arguments)]
	pub fn choice<T: Copy + PartialEq + 'static>(&mut self, id: &str, label: &str, help_text: Option<&str>, items: &[(T, &str, Option<&str>)], selected: T, vertical: bool, cards: bool, disabled: bool, set: impl Fn(&mut Self, T) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let set: Rc<dyn Fn(&mut Self, T)> = Rc::new(set);
		let mut group = div().flex().when(vertical, |group| group.flex_col().gap(px(2.))).when(!vertical && !cards, |group| group.flex_wrap().gap(px(4.))).when(cards, |group| group.gap(px(16.)));
		for (index, (value, item_label, description)) in items.iter().enumerate() {
			let value = *value;
			let checked = value == selected;
			let focus = self.focus_handle_for(&format!("{id}-{index}"), cx);
			let ring = Self::ring_visible(&focus, window);
			let set = Rc::clone(&set);
			let key_set = Rc::clone(&set);
			group = group.child(
				div()
					.id(ElementId::Name(format!("choice-{id}-{index}").into()))
					.relative()
					.flex()
					.flex_col()
					.items_start()
					.gap(px(4.))
					.border_l_4()
					.border_color(if checked { c(ACCENT) } else { gpui::transparent_black() })
					.when(!checked, |item| item.rounded(px(12.)).hover(|item| item.bg(ca(FIELD, 0.7))))
					.when(cards, |item| item.flex_1().min_w_0().px(px(10.)).py(px(7.)))
					.when(!cards, |item| item.px(px(8.)).py(px(4.)))
					.when(vertical, |item| item.w_full())
					.cursor_pointer()
					.track_focus(&focus)
					.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
						if !disabled {
							set(this, value);
							this.changed(cx);
						}
					}))
					.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
						if matches!(event.keystroke.key.as_str(), "space") && !disabled {
							key_set(this, value);
							this.changed(cx);
							cx.stop_propagation();
						}
					}))
					.child(div().xs().semibold().text_color(c(if checked { ACCENT_TEXT } else { FG })).child(SharedString::from((*item_label).to_owned())))
					.when_some(*description, |item, text| item.child(div().xs().regular().text_color(c(DIM)).child(SharedString::from(text.to_owned()))))
					.when(ring, |item| item.child(focus_ring(2.))),
			);
		}
		field().when(disabled, |frame| frame.opacity(0.5)).child(field_label(label.to_owned())).child(group).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}

	/// A `USelect` trigger in the window's soft skin, with its menu when open.
	pub fn select_box<T: Copy + PartialEq + 'static>(&mut self, id: &str, items: &[(T, String)], selected: T, disabled: bool, set: impl Fn(&mut Self, T) + 'static, window: &Window, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
		let popup_id: SharedString = format!("select-{id}").into();
		let open = matches!(&self.popup, Some(Popup::Select(open)) if *open == popup_id);
		let focus = self.focus_handle_for(&popup_id, cx);
		let ring = Self::ring_visible(&focus, window);
		let current = items.iter().find(|(value, _)| *value == selected).map(|(_, label)| label.clone()).unwrap_or_default();
		let set: Rc<dyn Fn(&mut Self, T)> = Rc::new(set);
		let toggle_id = popup_id.clone();
		let menu = open.then(|| {
			let mut list = div().flex().flex_col().p(px(4.)).gap(px(0.));
			for (index, (value, label)) in items.iter().enumerate() {
				let value = *value;
				let set = Rc::clone(&set);
				list = list.child(
					div()
						.id(ElementId::Name(format!("{popup_id}-{index}").into()))
						.flex()
						.items_center()
						.gap(px(6.))
						.px(px(8.))
						.py(px(6.))
						.rounded(px(6.))
						.xs()
						.text_color(c(BRIGHT))
						.hover(|item| item.bg(c(FIELD)))
						.cursor_pointer()
						.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
							set(this, value);
							this.popup = None;
							this.changed(cx);
						}))
						.child(div().flex_1().truncate().child(label.clone()))
						.when(value == selected, |item| item.child(icon("lucide--check", 16., c(BRIGHT)))),
				);
			}
			gpui::deferred(gpui::anchored().snap_to_window_with_margin(px(8.)).child(
				div()
					.id(ElementId::Name(format!("{popup_id}-menu").into()))
					.occlude()
					.mt(px(4.))
					.min_w(px(160.))
					.max_h(px(240.))
					.overflow_y_scroll()
					.rounded(px(9.))
					.bg(c(BG))
					.border_1()
					.border_color(c(RULE))
					.shadow_lg()
					.on_mouse_down_out(cx.listener(|this, _, _, cx| {
						this.popup = None;
						cx.notify();
					}))
					.child(list),
			))
			.with_priority(1)
		});
		div()
			.id(ElementId::Name(popup_id.clone()))
			.relative()
			.flex()
			.items_center()
			.h(px(28.))
			.pl(px(10.))
			.pr(px(32.))
			.py(px(6.))
			.rounded(px(9.))
			.bg(c(FIELD))
			.xs()
			.regular()
			.text_color(c(BRIGHT))
			.when(disabled, |trigger| trigger.opacity(0.75))
			.when(!disabled, |trigger| trigger.cursor_pointer())
			.track_focus(&focus)
			.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
				if !disabled {
					this.popup = if matches!(&this.popup, Some(Popup::Select(open)) if *open == toggle_id) { None } else { Some(Popup::Select(toggle_id.clone())) };
					cx.notify();
				}
			}))
			.child(div().truncate().child(current))
			.child(div().absolute().right(px(10.)).top(px(6.)).child(icon("lucide--chevron-down", 16., c(DIM))))
			.when(ring, |trigger| trigger.child(focus_ring(9.)))
			.children(menu)
	}

	/// `ControlSelect`: a labelled drop-down.
	#[allow(clippy::too_many_arguments)]
	pub fn select<T: Copy + PartialEq + 'static>(&mut self, id: &str, label: &str, help_text: Option<&str>, items: &[(T, String)], selected: T, disabled: bool, set: impl Fn(&mut Self, T) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let trigger = self.select_box(id, items, selected, disabled, set, window, cx);
		field().child(field_label(label.to_owned())).child(trigger).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}

	/// `ControlNumber`: a labelled number field with its unit.
	#[allow(clippy::too_many_arguments)]
	pub fn number(&mut self, id: &str, label: &str, help_text: Option<&str>, value: f64, min: f64, max: f64, decimals: usize, unit: Option<&str>, disabled: bool, set: impl Fn(&mut Self, f64) + 'static, cx: &mut Context<Self>) -> AnyElement {
		let state = self.number_input(id, value, decimals, min, max, Rc::new(set), cx);
		field()
			.when(disabled, |frame| frame.opacity(0.75))
			.child(field_label(label.to_owned()))
			.child(div().flex().items_center().gap(px(8.)).child(input(&state, cx).flex_1().min_w_0().h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE)).xs().text_color(c(FG))).when_some(unit, |row, unit| row.child(div().flex_none().text_color(c(DIM)).child(unit.to_owned()))))
			.when_some(help_text, |frame, text| frame.child(help(text.to_owned())))
			.into_any_element()
	}

	/// `ControlText`.
	pub fn text_field(&mut self, id: &str, label: &str, help_text: Option<&str>, placeholder: &str, value: &str, set: impl Fn(&mut Self, String) + 'static, cx: &mut Context<Self>) -> AnyElement {
		let key: SharedString = format!("text-{id}").into();
		let state = if let Some(state) = self.inputs.get(&key) {
			let state = state.clone();
			if state.read(cx).content() != value && self.editing.as_ref() != Some(&key) {
				let value = value.to_owned();
				state.update(cx, |state, cx| state.set_content_silent(value, cx));
			}
			state
		} else {
			let value = value.to_owned();
			let state = cx.new(|cx| {
				let mut state = InputState::new_singleline(cx);
				state.set_content_silent(value, cx);
				state
			});
			let editing_key = key.clone();
			self.subscriptions.push(cx.subscribe(&state, move |this, state, event: &gpuikit::input::InputStateEvent, cx| match event {
				gpuikit::input::InputStateEvent::Focus => this.editing = Some(editing_key.clone()),
				gpuikit::input::InputStateEvent::Blur => this.editing = None,
				gpuikit::input::InputStateEvent::TextChanged => {
					set(this, state.read(cx).content().to_owned());
					this.changed(cx);
				}
				_ => {}
			}));
			self.inputs.insert(key, state.clone());
			state
		};
		field()
			.child(field_label(label.to_owned()))
			.child(input(&state, cx).placeholder(placeholder.to_owned()).h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE)).xs().text_color(c(FG)))
			.when_some(help_text, |frame, text| frame.child(help(text.to_owned())))
			.into_any_element()
	}

	/// `ControlOptional`: a switch that gives or removes an option, and its value controls
	/// while it is given.
	#[allow(clippy::too_many_arguments)]
	pub fn optional(&mut self, id: &str, label: &str, help_text: Option<&str>, unset: Option<&str>, given: bool, set_given: impl Fn(&mut Self, bool) + 'static, body: Option<AnyElement>, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let shown_help = if given || unset.is_none() { help_text } else { unset };
		let toggle = self.toggle(id, label, shown_help, given, false, set_given, window, cx);
		div().flex().flex_col().min_w_0().child(toggle).when_some(body.filter(|_| given), |column, body| column.child(div().flex().flex_col().pl(px(20.)).child(body))).into_any_element()
	}

	/// `ControlDisclosure`: a section that folds away behind its heading.
	pub fn disclosure(&mut self, id: &str, title: &str, default_open: bool, children: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> Vec<AnyElement>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
		let key: SharedString = id.to_owned().into();
		let open = *self.disclosures.entry(key.clone()).or_insert(default_open);
		let focus = self.focus_handle_for(&format!("disclosure-{id}"), cx);
		let ring = Self::ring_visible(&focus, window);
		let body = open.then(|| children(self, window, cx));
		let toggle_key = key.clone();
		div()
			.flex()
			.flex_col()
			.w_full()
			.gap(px(16.))
			.child(
				div()
					.id(ElementId::Name(format!("disclosure-{id}").into()))
					.relative()
					.flex()
					.w_full()
					.items_center()
					.gap(px(8.))
					.sm()
					.semibold()
					.text_color(c(BRIGHT))
					.cursor_pointer()
					.track_focus(&focus)
					.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
						let open = this.disclosures.entry(toggle_key.clone()).or_insert(false);
						*open = !*open;
						cx.notify();
					}))
					.child(icon("subway--down-2", 16., c(BRIGHT)).when(!open, |chevron| chevron.with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2)))))
					.child(SharedString::from(title.to_owned()))
					.when(ring, |button| button.child(focus_ring(2.))),
			)
			.when_some(body, |section, body| section.child(div().flex().flex_col().gap(px(16.)).pb(px(8.)).children(body)))
			.into_any_element()
	}

	/// `ControlFile`: a file chosen by path.
	pub fn file(&mut self, id: &str, label: &str, help_text: Option<&str>, value: &str, set: impl Fn(&mut Self, String) + 'static, cx: &mut Context<Self>) -> AnyElement {
		let set = Rc::new(set);
		field()
			.child(field_label(label.to_owned()))
			.child(
				div()
					.flex()
					.items_center()
					.gap(px(8.))
					.child(div().min_w_0().flex_1().truncate().text_color(c(BRIGHT)).child(if value.is_empty() { "No file chosen".to_owned() } else { value.to_owned() }))
					.child(
						button_soft(&format!("file-{id}"), "Choose…", true).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
							let paths = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
							let set = Rc::clone(&set);
							cx.spawn(async move |this, cx| {
								if let Ok(Ok(Some(paths))) = paths.await
									&& let Some(path) = paths.into_iter().next()
								{
									this.update(cx, |this, cx| {
										set(this, path.display().to_string());
										this.changed(cx);
									})
									.ok();
								}
							})
							.detach();
						})),
					),
			)
			.when_some(help_text, |frame, text| frame.child(help(text.to_owned())))
			.into_any_element()
	}

	/// `ControlColor`: a colour as the `0xRRGGBB` number `cwebp -blend_alpha` takes.
	pub fn color(&mut self, label: &str, value: u32) -> AnyElement {
		field()
			.child(field_label(label.to_owned()))
			.child(
				div()
					.flex()
					.items_center()
					.gap(px(8.))
					// The webview's `<input type="color">`: a swatch that opens the platform's
					// colour chooser. gpui has no colour chooser, so this swatch only shows the
					// colour (see README.md, "Not possible").
					.child(div().size(px(28.)).rounded(px(6.)).p(px(4.)).child(div().size_full().rounded(px(2.)).bg(gpui::rgb(value)).border_1().border_color(c(RULE))))
					.child(div().text_color(c(BRIGHT)).child(format!("0x{value:06x}"))),
			)
			.into_any_element()
	}
}

/// A Nuxt UI `UButton` in `color="neutral" variant="soft"`, size `sm` or `xs`.
pub fn button_soft(id: &str, label: &str, extra_small: bool) -> gpui::Stateful<gpui::Div> {
	div()
		.id(ElementId::Name(id.to_owned().into()))
		.flex()
		.flex_none()
		.items_center()
		.gap(px(6.))
		.when(extra_small, |button| button.px(px(8.)).py(px(4.)).rounded(px(6.)))
		.when(!extra_small, |button| button.px(px(10.)).py(px(6.)).rounded(px(9.)))
		.bg(c(FIELD))
		.hover(|button| button.bg(ca(RULE, 0.75)))
		.xs()
		.medium()
		.text_color(c(FG))
		.cursor_pointer()
		.child(SharedString::from(label.to_owned()))
}

/// How wide `text` is in the window's 12px type at `weight`.
pub fn text_width(text: &str, weight: gpui::FontWeight, window: &Window) -> Pixels {
	let font = gpui::Font { family: crate::theme::FONT.into(), weight, ..gpui::font(crate::theme::FONT) };
	let run = gpui::TextRun { len: text.len(), font, color: c(FG), background_color: None, underline: None, strikethrough: None };
	window.text_system().shape_line(SharedString::from(text.to_owned()), px(12.), &[run], None).width
}

/// Snap `value` to `step` from `min`, inside the range.
#[must_use]
pub fn snap(value: f64, min: f64, max: f64, step: f64) -> f64 {
	let snapped = ((value - min) / step).round().mul_add(step, min);
	// Keep the step's own precision, so 0.1 steps do not drift into 0.30000000000000004.
	let places = step.to_string().split('.').nth(1).map_or(0, str::len);
	let factor = 10_f64.powi(i32::try_from(places).unwrap_or(0));
	((snapped * factor).round() / factor).clamp(min, max)
}

/// A number as the window shows it: `Intl.NumberFormat` with no grouping and up to
/// `decimals` fraction digits.
#[must_use]
pub fn format_number(value: f64, decimals: usize) -> String {
	let text = format!("{value:.decimals$}");
	if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
}

thread_local! {
	static PROBES: std::cell::RefCell<std::collections::BTreeMap<String, Bounds<Pixels>>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// With `SKID_PROBE` set, record its parent's bounds under `name`, for comparing the
/// layout with the old window's DOM. The parent must be `relative()`.
pub fn probe(name: &str) -> impl IntoElement {
	let name = name.to_owned();
	canvas(|_, _, _| (), move |bounds, (), _, _| {
		if std::env::var_os("SKID_PROBE").is_some() {
			PROBES.with(|probes| probes.borrow_mut().insert(name.clone(), bounds));
		}
	})
	.absolute()
	.size_full()
}

/// Print and clear what the probes recorded.
pub fn print_probes() {
	PROBES.with(|probes| {
		let probes = std::mem::take(&mut *probes.borrow_mut());
		if !probes.is_empty() {
			let mut lines: Vec<_> = probes.iter().collect();
			lines.sort_by(|a, b| a.1.top().partial_cmp(&b.1.top()).unwrap_or(std::cmp::Ordering::Equal));
			for (name, bounds) in lines {
				eprintln!("{name} {:.2} {:.2} {:.2}x{:.2}", f32::from(bounds.left()), f32::from(bounds.top()), f32::from(bounds.size.width), f32::from(bounds.size.height));
			}
			eprintln!("--");
		}
	});
}
