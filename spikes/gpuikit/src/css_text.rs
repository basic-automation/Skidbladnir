//! Text that wraps the way the webview wraps it.
//!
//! gpui's line wrapper differs from CSS in two ways that move words between lines in this
//! window's help text:
//!
//! - it counts the space after a word when deciding whether the word fits, where CSS lets
//!   that space hang past the edge, so a word that exactly fits goes to the next line;
//! - it treats any punctuation as a break opportunity, so `progressive/responsive` or
//!   `[enc/dec]` can split at the slash, where Unicode line breaking (UAX #14) forbids it.
//!
//! This element measures with gpui's own shaper and breaks lines as CSS `white-space:
//! normal` does for this text: collapse whitespace, break after a space, or after a hyphen
//! between a letter or digit and a letter; never inside anything else. A word longer than
//! the line overflows, as `overflow-wrap: normal` lets it.

use std::{cell::RefCell, rc::Rc};

use gpui::{
	App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Size, Style, TextAlign, TextStyle, Window, point, px, size
};

pub struct CssText {
	text: SharedString,
}

/// Text in the inherited style, wrapped as CSS wraps it.
pub fn css_text(text: impl Into<SharedString>) -> CssText {
	CssText { text: text.into() }
}

impl IntoElement for CssText {
	type Element = Self;

	fn into_element(self) -> Self::Element {
		self
	}
}

/// The words of `text` with whitespace collapsed, split where a line may break. Each piece
/// keeps the space that follows it, which hangs at a line's end.
fn pieces(text: &str) -> Vec<String> {
	let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
	let chars: Vec<char> = collapsed.chars().collect();
	let mut pieces = Vec::new();
	let mut current = String::new();
	for (index, &character) in chars.iter().enumerate() {
		current.push(character);
		let next = chars.get(index + 1).copied();
		let previous = index.checked_sub(1).map(|before| chars[before]);
		let breaks = match character {
			' ' => true,
			// UAX #14 class HY: a break after `-` in `a-b`, not in `-q` or `x-2`.
			'-' => previous.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphabetic),
			_ => false,
		};
		if breaks {
			pieces.push(std::mem::take(&mut current));
		}
	}
	if !current.is_empty() {
		pieces.push(current);
	}
	pieces
}

fn width_of(text: &str, style: &TextStyle, window: &mut Window) -> Pixels {
	let font_size = style.font_size.to_pixels(window.rem_size());
	let run = style.to_run(text.len());
	window.text_system().shape_line(SharedString::from(text.to_owned()), font_size, &[run], None).width
}

/// Break `text` into lines no wider than `width`, as CSS does.
fn lines(text: &str, width: Option<Pixels>, style: &TextStyle, window: &mut Window) -> Vec<String> {
	let mut lines = Vec::new();
	let mut line = String::new();
	for piece in pieces(text) {
		let candidate = format!("{line}{piece}");
		// Trailing spaces hang: only the ink has to fit.
		let fits = width.is_none_or(|width| width_of(candidate.trim_end(), style, window) <= width);
		if fits || line.is_empty() {
			line = candidate;
		} else {
			lines.push(std::mem::take(&mut line).trim_end().to_owned());
			line = piece;
		}
	}
	if !line.is_empty() || lines.is_empty() {
		lines.push(line.trim_end().to_owned());
	}
	lines
}

pub struct Layout {
	line_height: Pixels,
	/// The inherited style, taken while it is in scope: gpui measures later, outside it.
	style: TextStyle,
}

impl Element for CssText {
	type RequestLayoutState = Layout;
	type PrepaintState = ();

	fn id(&self) -> Option<ElementId> {
		None
	}

	fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
		None
	}

	fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, _: &mut App) -> (LayoutId, Self::RequestLayoutState) {
		let style = window.text_style();
		let line_height = style.line_height_in_pixels(window.rem_size());
		let text = self.text.clone();
		let measure_style = style.clone();
		let measured: Rc<RefCell<Option<(Option<Pixels>, Size<Pixels>)>>> = Rc::default();
		let layout_id = window.request_measured_layout(Style::default(), move |known, available, window, _| {
			let width = known.width.or(match available.width {
				gpui::AvailableSpace::Definite(width) => Some(width),
				_ => None,
			});
			if let Some((at, size)) = *measured.borrow()
				&& at == width
			{
				return size;
			}
			let lines = lines(&text, width, &measure_style, window);
			let widest = lines.iter().map(|line| width_of(line, &measure_style, window)).fold(px(0.), Pixels::max);
			#[allow(clippy::cast_precision_loss)]
			let result = size(width.map_or(widest, |width| widest.min(width)), line_height * lines.len() as f32);
			*measured.borrow_mut() = Some((width, result));
			result
		});
		(layout_id, Layout { line_height, style })
	}

	fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut Window, _: &mut App) -> Self::PrepaintState {}

	fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, layout: &mut Self::RequestLayoutState, (): &mut Self::PrepaintState, window: &mut Window, cx: &mut App) {
		let style = layout.style.clone();
		let font_size = style.font_size.to_pixels(window.rem_size());
		// Lines are broken against the box's own width, which is at least as wide as the
		// width measured for it.
		let lines = lines(&self.text, Some(bounds.size.width + px(0.01)), &style, window);
		for (index, line) in lines.into_iter().enumerate() {
			let run = style.to_run(line.len());
			let shaped = window.text_system().shape_line(SharedString::from(line), font_size, &[run], None);
			#[allow(clippy::cast_precision_loss)]
			let origin = point(bounds.left(), bounds.top() + layout.line_height * index as f32);
			let _ = shaped.paint(origin, layout.line_height, TextAlign::Left, None, window, cx);
		}
	}
}
