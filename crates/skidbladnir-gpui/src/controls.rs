//! The old window's controls (`frontend/components/Control*.vue`), rebuilt to the geometry
//! the 1.0.0 frontend computes. Each is a method on [`Skid`] that renders from the current
//! settings and writes back through a setter, the way `v-model` does: there is one copy of
//! every value, the settings struct.

use std::rc::Rc;

use gpui::{AnyElement, AppContext, Bounds, ClickEvent, Context, ElementId, FocusHandle, Focusable as _, Hsla, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, prelude::FluentBuilder, px, relative, svg};
use gpuikit::{elements::input::input, input::InputState};

use crate::{
	app::{Popup, Skid}, css_text::css_text, fade::FadeBg, theme::{ACCENT, ACCENT_TEXT, BG, BRIGHT, DIM, FG, FIELD, MUTED, RULE, Type, VIOLET, c, ca}
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

/// One labelled control's frame: `flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs`.
pub fn field() -> gpui::Div {
	div().flex().flex_col().min_w_0().gap(px(8.)).px(px(10.)).py(px(7.)).xs()
}

pub fn field_label(label: impl Into<SharedString>) -> gpui::Div {
	div().xs().semibold().text_color(c(FG)).child(css_text(label))
}

pub fn help(text: impl Into<SharedString>) -> gpui::Div {
	div().xs().regular().text_color(c(DIM)).child(css_text(text))
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
	#[expect(clippy::unused_self, reason = "a method like every other control, so panels read `this.panel(…)`")]
	pub fn panel(&self, title: &str, actions: Option<AnyElement>, children: Vec<AnyElement>) -> gpui::Div {
		div().flex()
			.flex_col()
			.w_full()
			.gap(px(16.))
			.child(div()
				.flex()
				.items_center()
				.gap(px(12.))
				// `<h2>`: a level-2 heading, for a screen reader's heading navigation.
				.child(div().id(ElementId::Name(format!("heading-{title}").into())).role(gpui::Role::Heading).aria_level(2).aria_label(title.to_owned()).relative().child(probe(&format!("h2 {title}"))).min_w_0().flex_1().sm().semibold().text_color(c(BRIGHT)).child(SharedString::from(title.to_owned())))
				.children(actions))
			.children(children)
	}

	/// `grid grid-cols-N gap-G`: equal columns, each child in its own cell.
	pub fn grid(columns: usize, gap: f32, children: Vec<AnyElement>) -> gpui::Div {
		let mut cells: Vec<AnyElement> = children.into_iter().map(|child| div().flex_1().min_w_0().flex().flex_col().child(child).into_any_element()).collect();
		while !cells.len().is_multiple_of(columns) {
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
		div().id(ElementId::Name(SharedString::from(format!("toggle-{id}"))))
			.flex()
			.items_start()
			.min_w_0()
			.px(px(10.))
			.py(px(7.))
			.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, cx)))
			.child(div().h(px(20.)).flex().items_center().mt(px(-2.)).child(div()
				.id(ElementId::Name(SharedString::from(format!("toggle-box-{id}"))))
				.role(gpui::Role::CheckBox)
				.aria_label(label.to_owned())
				.aria_toggled(if checked { gpui::Toggled::True } else { gpui::Toggled::False })
				.when_some(help_text, |box_, text| box_.aria_description(text.to_owned()))
				.relative()
				.size(px(16.))
				.rounded(px(4.))
				.bg(c(if checked { ACCENT } else { MUTED }))
				// Disabled, only the square dims (1.2.0): fading the label and help too
				// took the help under WCAG AA's 4.5:1.
				.when(disabled, |box_| box_.opacity(0.5))
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
				.when(ring, |box_| box_.child(Ring::Primary.element(4.)))))
			.child(div().ml(px(8.)).flex().flex_1().flex_col().gap(px(4.)).min_w_0().child(div().relative().child(probe(&format!("label {label}"))).xs().semibold().text_color(c(FG)).child(css_text(label.to_owned()))).when_some(help_text, |wrapper, text| wrapper.child(help(text.to_owned()).relative().child(probe(&format!("p {}", text.chars().take(24).collect::<String>()))))))
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
		#[expect(clippy::cast_possible_truncation, reason = "a fraction in 0..=1, drawn to a pixel")]
		let drawn = fraction as f32;

		let readout: AnyElement = if let Some(places) = decimals {
			let state = self.number_input(&id, value, places, min, max, Rc::clone(&set), cx);
			// UInputNumber here is `w-16 text-right`. gpuikit's input always draws its text
			// from the left, so the field is sized to its own text inside a right-aligned
			// 64px slot, which puts the digits where the old window puts them.
			let width = text_width(state.read(cx).content(), crate::theme::SEMIBOLD, window);
			let field = named_input(&state, &format!("{id}-exact"), &format!("{label}, exact value"), None, cx).w(width).h(px(16.)).xs().semibold().text_color(c(ACCENT_TEXT)).bg(gpui::transparent_black());
			self.stepping(div().w(px(64.)).flex().justify_end().child(field), &state, value, min, max, step, places, Rc::clone(&set), cx).into_any_element()
		} else {
			let text = divisor.map_or_else(|| format_number(value, 0), |divisor| format!("{:.1}", value / divisor));
			div().text_color(c(if disabled { DIM } else { ACCENT_TEXT })).child(text).into_any_element()
		};

		let bounds_store = Rc::clone(&self.slider_bounds);
		let store_id = id.clone();
		let drag_set = Rc::clone(&set);
		let drag_id = id.clone();
		let key_set = Rc::clone(&set);
		let thumb = 14.0_f32;

		field().child(div().flex().items_baseline().gap(px(12.)).xs().semibold().child(div().min_w_0().flex_1().text_color(c(FG)).child(SharedString::from(label.to_owned())).when(disabled, |label| label.child(div().regular().text_color(c(DIM)).child(" · not in use")))).child(readout))
			.child(div()
				.id(ElementId::Name(format!("slider-{id}").into()))
				.relative()
				.h(px(7.))
				.w_full()
				.when(disabled, |track| track.opacity(0.5))
				.child(canvas(
					|bounds, _, _| bounds,
					move |_, bounds: Bounds<Pixels>, _, _| {
						bounds_store.borrow_mut().insert(store_id.clone(), bounds);
					},
				)
				.absolute()
				.size_full())
				.child(div().absolute().inset_0().rounded_full().bg(c(DIM)))
				.child(div().absolute().top_0().bottom_0().left_0().w(relative(drawn)).rounded_full().bg(c(ACCENT)))
				.child(
					// Reka keeps the thumb inside the track: its centre moves from half a thumb
					// in at the minimum to half a thumb in at the maximum.
					div().absolute().top(px(3.5 - thumb / 2. - 2.)).left(relative(drawn)).ml(px((0.5 - drawn) * thumb - thumb / 2. - 2.)).size(px(thumb + 4.)).rounded_full().border_2().border_color(c(ACCENT)).bg(c(BG)).id(ElementId::Name(format!("slider-thumb-{id}").into())).role(gpui::Role::Slider).aria_label(label.to_owned()).aria_numeric_value(value).aria_min_numeric_value(min).aria_max_numeric_value(max).aria_numeric_value_step(step).aria_orientation(gpui::Orientation::Horizontal).when_some(help_text, |thumb_, text| thumb_.aria_description(text.to_owned())).track_focus(&focus).when(ring, |thumb_| thumb_.child(crate::controls::ring(3., ca(ACCENT, 0.25), 0., 9.))).on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
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
				}))
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
			if self.editing.as_ref().is_none_or(|editing| *editing != key) && state.read(cx).content() != text {
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
		let set: Set<T> = Rc::new(set);
		// Reka's radio group is one tab stop, the chosen item, and the arrow keys move the
		// choice within it.
		let values: Rc<Vec<T>> = Rc::new(items.iter().map(|(value, _, _)| *value).collect());
		let handles: Rc<Vec<FocusHandle>> = Rc::new((0..items.len()).map(|index| self.focus_handle_for(&format!("{id}-{index}"), cx)).collect());
		let current = values.iter().position(|value| *value == selected);
		for (index, handle) in handles.iter().enumerate() {
			let _ = handle.clone().tab_stop(current.map_or(index == 0, |current| current == index));
		}
		let size = items.len();
		let mut group = div().id(ElementId::Name(format!("choice-group-{id}").into())).role(gpui::Role::RadioGroup).aria_label(label.to_owned()).flex().when(vertical, |group| group.flex_col().gap(px(2.))).when(!vertical && !cards, |group| group.flex_wrap().gap(px(4.))).when(cards, |group| group.gap(px(16.)));
		for (index, (value, item_label, description)) in items.iter().enumerate() {
			let value = *value;
			let checked = value == selected;
			let focus = self.focus_handle_for(&format!("{id}-{index}"), cx);
			let ring = Self::ring_visible(&focus, window);
			let set = Rc::clone(&set);
			let key_set = Rc::clone(&set);
			group = group.child(div()
				.id(ElementId::Name(format!("choice-{id}-{index}").into()))
				.role(gpui::Role::RadioButton)
				.aria_label((*item_label).to_owned())
				.aria_toggled(if checked { gpui::Toggled::True } else { gpui::Toggled::False })
				.aria_position_in_set(index + 1)
				.aria_size_of_set(size)
				.when_some(*description, |item, text| item.aria_description(text.to_owned()))
				.relative()
				.flex()
				.flex_col()
				.items_start()
				.gap(px(4.))
				.border_l_4()
				.border_color(if checked || ring { c(ACCENT) } else { gpui::transparent_black() })
				.when(!checked, |item| item.rounded(px(12.)).when(!ring, |item| item.fade_bg_clear(format!("choice-{id}-{index}"), ca(FIELD, 0.7))))
				.when(cards, |item| item.flex_1().min_w_0().px(px(10.)).py(px(7.)))
				.when(!cards, |item| item.px(px(8.)).py(px(4.)))
				.when(vertical, gpui::Styled::w_full)
				.cursor_pointer()
				.track_focus(&focus)
				.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
					if !disabled {
						set(this, value);
						this.changed(cx);
					}
				}))
				.on_key_down(cx.listener({
					let values = Rc::clone(&values);
					let handles = Rc::clone(&handles);
					move |this, event: &KeyDownEvent, window, cx| {
						if disabled {
							return;
						}
						let target = match event.keystroke.key.as_str() {
							"space" => index,
							"right" | "down" => (index + 1) % values.len(),
							"left" | "up" => (index + values.len() - 1) % values.len(),
							_ => return,
						};
						key_set(this, values[target]);
						window.focus(&handles[target], cx);
						this.changed(cx);
						cx.stop_propagation();
					}
				}))
				.child(div().xs().semibold().text_color(c(if checked { ACCENT_TEXT } else { FG })).child(css_text((*item_label).to_owned())))
				.when_some(*description, |item, text| item.child(div().xs().regular().text_color(c(DIM)).child(css_text(text.to_owned()))))
				.when(ring, |item| item.child(crate::controls::ring(3., ca(ACCENT, 0.25), 0., if checked { 0. } else { 12. }))));
		}
		field().when(disabled, |frame| frame.opacity(0.5)).child(field_label(label.to_owned())).child(group).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}

	/// A `USelect` trigger in the window's soft skin, with its menu when open.
	#[expect(clippy::too_many_lines, reason = "USelect's trigger and menu, with Reka's placement and keys")]
	#[allow(clippy::too_many_arguments)]
	pub fn select_box<T: Copy + PartialEq + 'static>(&mut self, id: &str, name: &str, items: &[(T, String)], selected: T, disabled: bool, set: impl Fn(&mut Self, T) + 'static, window: &Window, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
		let popup_id: SharedString = format!("select-{id}").into();
		let open = matches!(&self.popup, Some(Popup::Select(open)) if *open == popup_id);
		let focus = self.focus_handle_for(&popup_id, cx);
		let ring = Self::ring_visible(&focus, window);
		let current = items.iter().find(|(value, _)| *value == selected).map_or_default(|(_, label)| label.clone());
		let set: Set<T> = Rc::new(set);
		let toggle_id = popup_id.clone();
		let key_id = popup_id.clone();
		let selected_index = items.iter().position(|(value, _)| *value == selected).unwrap_or(0);
		let menu = open.then(|| {
			// USelect's menu: the trigger's width, 8px below it, or above it when there is not
			// room below and there is more above, as Reka's collision handling decides.
			let trigger = self.bounds_of(&popup_id);
			let mut list = div().id(ElementId::Name(format!("{popup_id}-options").into())).role(gpui::Role::ListBox).aria_label(name.to_owned()).flex().flex_col().p(px(4.));
			let mut height: f32 = 8.;
			let highlight = self.menu_highlight;
			self.menu_actions.clear();
			for (index, (value, label)) in items.iter().enumerate() {
				let value = *value;
				let chosen = value == selected;
				let highlighted = highlight == Some(index);
				height += if chosen { 28. } else { 27. };
				let set = Rc::clone(&set);
				let action_set = Rc::clone(&set);
				self.menu_actions.push(Rc::new(move |this: &mut Skid, _: &mut Window, cx: &mut Context<Skid>| {
					action_set(this, value);
					this.popup = None;
					this.changed(cx);
				}));
				list = list.child(div()
					.id(ElementId::Name(format!("{popup_id}-{index}").into()))
					.role(gpui::Role::ListBoxOption)
					.aria_label(label.clone())
					.aria_selected(chosen)
					.aria_position_in_set(index + 1)
					.aria_size_of_set(items.len())
					// The highlighted option is the one a screen reader announces, while focus
					// stays on the trigger.
					.when(highlighted, gpui::StatefulInteractiveElement::aria_active_descendant)
					.relative()
					.flex()
					.items_start()
					.gap(px(6.))
					.p(px(6.))
					.xs()
					.text_color(crate::fade::mix(c(FG), c(BRIGHT), crate::fade::fade(format!("{popup_id}-{index}"), highlighted)))
					.child(div().absolute().top(px(1.)).left(px(1.)).right(px(1.)).bottom(px(1.)).rounded(px(9.)).bg(ca(FIELD, 0.5 * crate::fade::fade(format!("{popup_id}-{index}"), highlighted))))
					.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
						if *hovered {
							this.menu_highlight = Some(index);
							cx.notify();
						}
					}))
					.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
						set(this, value);
						this.popup = None;
						this.changed(cx);
					}))
					.child(div().relative().flex_1().min_w_0().truncate().child(label.clone()))
					.when(chosen, |item| item.child(icon("lucide--check", 16., c(FG)))));
			}
			let height = height.min(240.);
			let (anchor, at) = match trigger {
				Some(trigger) => {
					let below = f32::from(window.viewport_size().height - trigger.bottom()) - 8.;
					let above = f32::from(trigger.top()) - 8.;
					// Relative to the trigger's top-left corner, so the menu scrolls with it.
					if below < height && above > below { (gpui::Anchor::BottomLeft, gpui::point(px(0.), px(-8.))) } else { (gpui::Anchor::TopLeft, gpui::point(px(0.), trigger.size.height + px(8.))) }
				}
				None => (gpui::Anchor::TopLeft, gpui::point(px(0.), px(0.))),
			};
			let width = trigger.map(|trigger| trigger.size.width);
			place_local(anchor, at, popup_panel(&format!("{popup_id}-menu"), width, div().id(ElementId::Name(format!("{popup_id}-list").into())).max_h(px(240.)).overflow_y_scroll().child(list), cx))
		});
		div().id(ElementId::Name(popup_id.clone()))
			.role(gpui::Role::ComboBox)
			.aria_label(name.to_owned())
			.aria_value(current.clone())
			.aria_expanded(open)
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
			.child(self.record(&popup_id))
			.track_focus(&focus)
			.on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
				if !disabled && !std::mem::take(&mut this.swallow_click) {
					this.popup = if matches!(&this.popup, Some(Popup::Select(open)) if *open == toggle_id) { None } else { Some(Popup::Select(toggle_id.clone())) };
					// Opened from the keyboard, Reka highlights the chosen item.
					this.menu_highlight = matches!(event, ClickEvent::Keyboard(_)).then_some(selected_index);
					cx.notify();
				}
			}))
			.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
				let closed = !matches!(&this.popup, Some(Popup::Select(open)) if *open == key_id);
				if closed && !disabled && matches!(event.keystroke.key.as_str(), "down" | "up") {
					this.popup = Some(Popup::Select(key_id.clone()));
					this.menu_highlight = Some(selected_index);
					cx.stop_propagation();
					cx.notify();
				}
			}))
			.child(div().truncate().child(current))
			.child(div().absolute().right(px(10.)).top(px(6.)).child(icon("lucide--chevron-down", 16., c(DIM))))
			.when(ring, |trigger| trigger.child(Ring::Primary.element(9.)))
			.children(menu)
	}

	/// `ControlSelect`: a labelled drop-down.
	#[allow(clippy::too_many_arguments)]
	pub fn select<T: Copy + PartialEq + 'static>(&mut self, id: &str, label: &str, help_text: Option<&str>, items: &[(T, String)], selected: T, disabled: bool, set: impl Fn(&mut Self, T) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let trigger = self.select_box(id, label, items, selected, disabled, set, window, cx);
		field().child(field_label(label.to_owned())).child(trigger).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}

	/// `ControlNumber`: a labelled number field with its unit.
	#[allow(clippy::too_many_arguments)]
	pub fn number(&mut self, id: &str, label: &str, help_text: Option<&str>, value: f64, min: f64, max: f64, decimals: usize, unit: Option<&str>, disabled: bool, set: impl Fn(&mut Self, f64) + 'static, cx: &mut Context<Self>) -> AnyElement {
		let set: Set<f64> = Rc::new(set);
		let state = self.number_input(id, value, decimals, min, max, Rc::clone(&set), cx);
		let field_ = named_input(&state, &format!("number-{id}"), label, None, cx).flex_1().min_w_0().h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE)).xs().text_color(c(FG));
		let row = self.stepping(div().flex().items_center().gap(px(8.)).child(field_), &state, value, min, max, 1., decimals, set, cx);
		field().when(disabled, |frame| frame.opacity(0.75)).child(field_label(label.to_owned())).child(row.when_some(unit, |row, unit| row.child(div().flex_none().text_color(c(DIM)).child(unit.to_owned())))).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}

	/// `ControlText`.
	#[expect(clippy::too_many_arguments, reason = "ControlText's props, one argument each, as the other controls take theirs")]
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
		field().child(field_label(label.to_owned())).child(named_input(&state, &format!("text-{id}"), label, Some(placeholder), cx).h(px(28.)).px(px(10.)).py(px(6.)).rounded(px(9.)).bg(c(RULE)).xs().text_color(c(FG))).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
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
		div().flex()
			.flex_col()
			.w_full()
			.gap(px(16.))
			.child(div()
				.id(ElementId::Name(format!("disclosure-{id}").into()))
				.role(gpui::Role::Button)
				.aria_label(title.to_owned())
				.aria_expanded(open)
				.relative()
				.flex()
				.w_full()
				.items_center()
				.gap(px(8.))
				.sm()
				.semibold()
				.text_color(c(BRIGHT))
				.track_focus(&focus)
				.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
					let open = this.disclosures.entry(toggle_key.clone()).or_insert(false);
					*open = !*open;
					cx.notify();
				}))
				.child(icon("subway--down-2", 16., c(BRIGHT)).when(!open, |chevron| chevron.with_transformation(gpui::Transformation::rotate(gpui::radians(-std::f32::consts::FRAC_PI_2)))))
				.child(SharedString::from(title.to_owned()))
				.when(ring, |button| button.child(Ring::Violet.element(2.))))
			.when_some(body, |section, body| section.child(div().flex().flex_col().gap(px(16.)).pb(px(8.)).children(body)))
			.into_any_element()
	}

	/// `ControlFile`: a file chosen by path.
	#[allow(clippy::too_many_arguments)]
	pub fn file(&mut self, id: &str, label: &str, help_text: Option<&str>, value: &str, set: impl Fn(&mut Self, String) + 'static, window: &Window, cx: &mut Context<Self>) -> AnyElement {
		let set = Rc::new(set);
		let choose = button_soft(&format!("file-{id}"), "Choose…", true).press(
			self,
			&format!("file-{id}"),
			&format!("Choose a file for {label}"),
			Ring::Neutral,
			6.,
			move |_, _, cx| {
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
			},
			window,
			cx,
		);
		field().child(field_label(label.to_owned())).child(div().flex().items_center().gap(px(8.)).child(div().min_w_0().flex_1().truncate().text_color(c(BRIGHT)).child(if value.is_empty() { "No file chosen".to_owned() } else { value.to_owned() })).child(choose)).when_some(help_text, |frame, text| frame.child(help(text.to_owned()))).into_any_element()
	}
}

/// A Nuxt UI `UButton` in `color="neutral" variant="soft"`, size `sm` or `xs`.
pub fn button_soft(id: &str, label: &str, extra_small: bool) -> gpui::Stateful<gpui::Div> {
	div().id(ElementId::Name(id.to_owned().into())).flex().flex_none().items_center().gap(px(6.)).when(extra_small, |button| button.px(px(8.)).py(px(4.)).rounded(px(6.))).when(!extra_small, |button| button.px(px(10.)).py(px(6.)).rounded(px(9.))).bg(c(FIELD)).fade_bg(id.to_owned(), c(FIELD), ca(RULE, 0.75)).xs().medium().text_color(c(FG)).child(SharedString::from(label.to_owned()))
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
	canvas(
		|_, _, _| (),
		move |bounds, (), _, _| {
			if std::env::var_os("SKID_PROBE").is_some() {
				PROBES.with(|probes| probes.borrow_mut().insert(name.clone(), bounds));
			}
		},
	)
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

/// Which focus outline a control draws, as each Nuxt UI component (or the base stylesheet)
/// defines it. Measured from the old window with each control focused from the keyboard.
#[derive(Clone, Copy)]
pub enum Ring {
	/// `outline-3 outline-primary/25 outline-offset-2`: primary buttons, selects, checkboxes.
	Primary,
	/// `outline-3 outline-inverted/25 outline-offset-2`: neutral buttons.
	Neutral,
	/// `outline-3 outline-error/25`: the error button.
	Error,
	/// The base `:focus-visible` rule as these buttons compute it: 2px of their own text
	/// colour, 2px out.
	Plain(u32),
	/// The base rule untouched: 2px violet, 2px out, and a 2px radius.
	Violet,
}

/// An outline `width` wide, `offset` outside an element whose corners are `radius`.
pub fn ring(width: f32, colour: Hsla, offset: f32, radius: f32) -> gpui::Div {
	let out = offset + width;
	div().absolute().top(px(-out)).left(px(-out)).right(px(-out)).bottom(px(-out)).border(px(width)).border_color(colour).rounded(px(radius + out))
}

impl Ring {
	pub fn element(self, radius: f32) -> gpui::Div {
		match self {
			Self::Primary => ring(3., ca(ACCENT, 0.25), 2., radius),
			Self::Neutral => ring(3., ca(BRIGHT, 0.25), 2., radius),
			Self::Error => ring(3., ca(crate::theme::ERROR, 0.25), 2., radius),
			Self::Plain(colour) => ring(2., c(colour), 2., radius),
			Self::Violet => ring(2., c(VIOLET), 2., 2.),
		}
	}
}

/// A button: focusable, pressed by a click or by Enter or Space, with its focus outline.
pub trait Press: Sized {
	#[allow(clippy::too_many_arguments)]
	fn press(self, skid: &mut Skid, id: &str, name: &str, ring: Ring, radius: f32, handler: impl Fn(&mut Skid, &mut Window, &mut Context<Skid>) + 'static, window: &Window, cx: &mut Context<Skid>) -> Self;
}

impl Press for gpui::Stateful<gpui::Div> {
	fn press(self, skid: &mut Skid, id: &str, name: &str, ring: Ring, radius: f32, handler: impl Fn(&mut Skid, &mut Window, &mut Context<Skid>) + 'static, window: &Window, cx: &mut Context<Skid>) -> Self {
		let focus = skid.focus_handle_for(&format!("press-{id}"), cx);
		// A dropdown menu takes focus into itself when it opens, so its trigger shows no
		// outline meanwhile; a select's trigger keeps it.
		let menu_has_focus = id == "queue" && skid.popup == Some(Popup::QueueMenu);
		let visible = Skid::ring_visible(&focus, window) && !menu_has_focus;
		// gpui clicks a focused element itself on Enter or Space (on key-up), so the click
		// handler is the keyboard handler too.
		// A button, named: gpui does not name a node from the text inside it.
		// No `.relative()` here: the ring is absolute within this element whatever its own
		// position, and a control that is itself absolute (a preset's delete cross) must stay so.
		self.role(gpui::Role::Button)
			.aria_label(name.to_owned())
			.track_focus(&focus)
			// A press on a control never reaches a drag region around it, as Tauri drags only
			// when the press lands on the region itself.
			.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
			.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
				if !std::mem::take(&mut this.swallow_click) {
					handler(this, window, cx);
				}
			}))
			.when(visible, |button| button.child(ring.element(radius)))
	}
}

/// A pop-up's panel: Nuxt UI's popover and menu content, `bg-default ring ring-default
/// rounded-md shadow-lg`. The ring is a box-shadow outside the box, so it is drawn outside
/// here too rather than as a border that would take a pixel from the content.
pub fn popup_panel(id: &str, width: Option<Pixels>, body: impl IntoElement, cx: &mut Context<Skid>) -> gpui::Stateful<gpui::Div> {
	div().id(ElementId::Name(id.to_owned().into()))
		.occlude()
		.relative()
		.when_some(width, gpui::Styled::w)
		.rounded(px(9.))
		.bg(c(BG))
		.shadow_lg()
		.on_mouse_down_out(cx.listener(|this, _, _, cx| {
			this.popup = None;
			cx.notify();
		}))
		.child(div().absolute().top(px(-1.)).left(px(-1.)).right(px(-1.)).bottom(px(-1.)).rounded(px(10.)).border_1().border_color(c(RULE)))
		.child(body)
}

/// Put a pop-up over the window with its `anchor` corner at `at`, relative to the top-left
/// corner of the element it is a child of (which must be `relative()`).
///
/// Inside the scrolling settings column a pop-up must be placed this way: one placed in
/// window coordinates there is moved again by the scroll offset.
pub fn place_local(anchor: gpui::Anchor, at: gpui::Point<Pixels>, panel: impl IntoElement) -> gpui::Div {
	div().absolute().top_0().left_0().child(gpui::deferred(gpui::anchored().position_mode(gpui::AnchoredPositionMode::Local).anchor(anchor).position(at).snap_to_window_with_margin(px(8.)).child(panel)).with_priority(1))
}

/// Put a pop-up over the window with its `anchor` corner at `at`, in window coordinates.
pub fn place(anchor: gpui::Anchor, at: gpui::Point<Pixels>, panel: impl IntoElement) -> gpui::Deferred {
	gpui::deferred(gpui::anchored().anchor(anchor).position(at).snap_to_window_with_margin(px(8.)).child(panel)).with_priority(1)
}

impl Skid {
	/// Record this element's bounds under `id` each frame; the parent must be `relative()`.
	pub fn record(&self, id: &str) -> impl IntoElement {
		let store = Rc::clone(&self.slider_bounds);
		let id: SharedString = id.to_owned().into();
		canvas(
			|_, _, _| (),
			move |bounds, (), window, _| {
				// Pop-ups are placed from last frame's bounds; when they move (a scroll, a resize),
				// draw once more so anything placed against them follows at once.
				if store.borrow_mut().insert(id.clone(), bounds) != Some(bounds) {
					window.refresh();
				}
			},
		)
		.absolute()
		.top_0()
		.left_0()
		.size_full()
	}

	pub fn bounds_of(&self, id: &str) -> Option<Bounds<Pixels>> {
		self.slider_bounds.borrow().get(id).copied()
	}
}

/// `UButton color="neutral" variant="ghost" icon=…`, size md: an icon-only button with a name.
#[allow(clippy::too_many_arguments)]
pub fn icon_button(skid: &mut Skid, id: &str, icon_name: &str, label: &str, handler: impl Fn(&mut Skid, &mut Window, &mut Context<Skid>) + 'static, window: &Window, cx: &mut Context<Skid>) -> gpui::Stateful<gpui::Div> {
	div().id(ElementId::Name(id.to_owned().into())).flex_none().p(px(6.)).rounded(px(9.)).fade_bg_clear(id.to_owned(), c(FIELD)).child(icon(icon_name, 20., c(FG))).press(skid, id, label, Ring::Neutral, 9., handler, window, cx)
}

/// `UButton size="sm" color="neutral" variant="soft" icon="i-material-symbols-add"`.
pub fn add_button(skid: &mut Skid, id: &str, label: &str, handler: impl Fn(&mut Skid, &mut Window, &mut Context<Skid>) + 'static, window: &Window, cx: &mut Context<Skid>) -> gpui::Stateful<gpui::Div> {
	div().id(ElementId::Name(id.to_owned().into())).flex().flex_none().items_center().gap(px(6.)).px(px(10.)).py(px(6.)).rounded(px(9.)).fade_bg(id.to_owned(), c(FIELD), ca(RULE, 0.75)).xs().medium().text_color(c(FG)).child(icon("material-symbols--add", 16., c(FG))).child(SharedString::from(label.to_owned())).press(skid, id, label, Ring::Neutral, 9., handler, window, cx)
}

/// A gpuikit input announced as a named text field. gpuikit's input has no accessibility
/// builders of its own, but any interactive element given an id has gpui's.
pub fn named_input(state: &gpui::Entity<InputState>, id: &str, name: &str, placeholder: Option<&str>, cx: &gpui::App) -> gpui::Stateful<gpuikit::elements::input::Input> {
	let field = input(state, cx);
	let field = match placeholder {
		Some(text) => field.placeholder(text.to_owned()),
		None => field,
	};
	field.id(ElementId::Name(format!("input-{id}").into())).role(gpui::Role::TextInput).aria_label(name.to_owned()).aria_value(state.read(cx).content().to_owned())
}

impl Skid {
	/// Reka's number field keys on a field: the arrows step it, Page Up and Page Down step
	/// it ten times, Home and End take it to its ends. Caught before the input sees them.
	#[allow(clippy::too_many_arguments)]
	#[expect(clippy::unused_self, reason = "a method like the controls that call it")]
	pub fn stepping(&self, row: gpui::Div, state: &gpui::Entity<InputState>, value: f64, min: f64, max: f64, step: f64, decimals: usize, set: Set<f64>, cx: &mut Context<Self>) -> gpui::Div {
		let state = state.clone();
		row.capture_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
			if !state.read(cx).focus_handle(cx).is_focused(window) {
				return;
			}
			let next = match event.keystroke.key.as_str() {
				"up" => value + step,
				"down" => value - step,
				"pageup" => value + step * 10.,
				"pagedown" => value - step * 10.,
				"home" if min.is_finite() && min > f64::MIN => min,
				"end" if max.is_finite() && max < f64::MAX => max,
				_ => return,
			};
			let next = next.clamp(min, max);
			set(this, next);
			state.update(cx, |state, cx| state.set_content_silent(format_number(next, decimals), cx));
			this.changed(cx);
			cx.stop_propagation();
		}))
	}
}
