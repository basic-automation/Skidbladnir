//! Tailwind's `transition-colors`: a colour change eased over 150ms with
//! `cubic-bezier(0.4, 0, 0.2, 1)`, reversing from wherever it has got to when the target
//! changes back, as a CSS transition does.
//!
//! gpui's `.hover()` style switches instantly. Here each faded element records whether it is
//! hovered (or highlighted), the colour is mixed by the eased progress at render time, and
//! the window asks for animation frames while any fade is still moving.

use std::{
	cell::{Cell, RefCell}, collections::HashMap, time::{Duration, Instant}
};

use gpui::{Div, Hsla, Rgba, SharedString, Stateful, StatefulInteractiveElement, Styled};

const DURATION: Duration = Duration::from_millis(150);

struct Fade {
	active: bool,
	/// Where the value was when `active` last changed, and when that was.
	from: f32,
	since: Instant,
}

thread_local! {
	static FADES: RefCell<HashMap<SharedString, Fade>> = RefCell::new(HashMap::new());
	static HOVERED: RefCell<HashMap<SharedString, bool>> = RefCell::new(HashMap::new());
	static MOVING: Cell<bool> = const { Cell::new(false) };
}

/// `cubic-bezier(0.4, 0, 0.2, 1)` at time `t`, solved for x by bisection.
fn ease(t: f32) -> f32 {
	let bezier = |a: f32, b: f32, s: f32| 3. * (1. - s) * (1. - s) * s * a + 3. * (1. - s) * s * s * b + s * s * s;
	let (mut low, mut high) = (0_f32, 1_f32);
	for _ in 0..24 {
		let mid = (low + high) / 2.;
		if bezier(0.4, 0.2, mid) < t { low = mid } else { high = mid }
	}
	bezier(0., 1., (low + high) / 2.)
}

fn value(fade: &Fade) -> f32 {
	let progress = (fade.since.elapsed().as_secs_f32() / DURATION.as_secs_f32()).min(1.);
	let target = if fade.active { 1. } else { 0. };
	fade.from + (target - fade.from) * ease(progress)
}

/// How far the colour under `key` has moved toward its active state, 0 to 1.
pub fn fade(key: impl Into<SharedString>, active: bool) -> f32 {
	let key = key.into();
	FADES.with(|fades| {
		let mut fades = fades.borrow_mut();
		let fade = fades.entry(key).or_insert(Fade { active, from: if active { 1. } else { 0. }, since: Instant::now() - DURATION });
		if fade.active != active {
			fade.from = value(fade);
			fade.active = active;
			fade.since = Instant::now();
		}
		if fade.since.elapsed() < DURATION {
			MOVING.with(|moving| moving.set(true));
		}
		value(fade)
	})
}

/// Whether any fade is still moving; clears the flag for the next frame.
pub fn take_moving() -> bool {
	MOVING.with(|moving| moving.replace(false))
}

/// `a` to `b` by `t`, interpolating premultiplied, as CSS does.
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
	let (a, b): (Rgba, Rgba) = (a.into(), b.into());
	let alpha = a.a + (b.a - a.a) * t;
	let channel = |x: f32, xa: f32, y: f32, ya: f32| {
		let premultiplied = x * xa + (y * ya - x * xa) * t;
		if alpha > 0. { premultiplied / alpha } else { 0. }
	};
	Rgba { r: channel(a.r, a.a, b.r, b.a), g: channel(a.g, a.a, b.g, b.a), b: channel(a.b, a.a, b.b, b.a), a: alpha }.into()
}

/// A background that fades to another on hover.
pub trait FadeBg: Sized {
	fn fade_bg(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self;

	/// From no background (`transparent`) to `hover`.
	fn fade_bg_clear(self, key: impl Into<SharedString>, hover: Hsla) -> Self {
		self.fade_bg(key, gpui::transparent_black(), hover)
	}
}

impl FadeBg for Stateful<Div> {
	fn fade_bg(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self {
		let key = key.into();
		let hovered = HOVERED.with(|hovered| hovered.borrow().get(&key).copied().unwrap_or(false));
		let t = fade(key.clone(), hovered);
		self.bg(mix(rest, hover, t)).on_hover(move |hovered, window, _| {
			HOVERED.with(|map| map.borrow_mut().insert(key.clone(), *hovered));
			window.refresh();
		})
	}
}
