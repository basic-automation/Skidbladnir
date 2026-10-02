//! `AboutPanel.vue`: what this build is and the licences it is distributed under — the
//! version, the edition, and the texts every installer carries (LICENSE, the third-party
//! notices and, in the GPL edition, COPYING). Compiled in, so they are always there.

use gpui::{AnyElement, Context, ElementId, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px};
use skidbladnir_encode::settings::HEIC_X265;

use crate::{
	app::{Skid, app_version}, controls::{Press, Ring, button_soft, icon}, css_text::css_text, theme::{ACCENT_TEXT, BRIGHT, DIM, FG, FIELD, Type, c}
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Document {
	License,
	Notices,
	Copying,
}

impl Document {
	const fn label(self) -> &'static str {
		match self {
			Self::License => "Skidbladnir licence",
			Self::Notices => "Third-party notices",
			Self::Copying => "GNU GPL v3",
		}
	}

	const fn text(self) -> &'static str {
		match self {
			Self::License => include_str!("../../../LICENSE"),
			Self::Notices if HEIC_X265 => include_str!("../../../THIRD-PARTY-NOTICES-GPL.md"),
			Self::Notices => include_str!("../../../THIRD-PARTY-NOTICES.md"),
			Self::Copying => include_str!("../../../LICENSES/GPL-3.0.txt"),
		}
	}
}

pub fn render(this: &mut Skid, window: &mut Window, cx: &mut Context<Skid>) -> AnyElement {
	let (label, license) = if HEIC_X265 { (" (GPL edition)", "GPL-3.0-or-later") } else { ("", "ISC") };
	let documents: &[Document] = if HEIC_X265 { &[Document::License, Document::Notices, Document::Copying] } else { &[Document::License, Document::Notices] };
	let shown = this.about_document;
	let close = div()
		.id("about-close")
		.p(px(4.))
		.rounded(px(9.))
		.child(icon("material-symbols--close", 16., c(FG)))
		.press(
			this,
			"about-close",
			"Close About",
			Ring::Neutral,
			9.,
			|t, _, cx| {
				t.about_open = false;
				cx.notify();
			},
			window,
			cx,
		)
		.into_any_element();
	let mut buttons = div().flex().flex_wrap().gap(px(8.)).pt(px(4.));
	for document in documents {
		let document = *document;
		let pressed = shown == Some(document);
		buttons = buttons.child(button_soft(&format!("about-{}", document.label()), document.label(), false)
			.when(pressed, |button| button.border_1().border_color(c(ACCENT_TEXT)))
			.press(
				this,
				&format!("about-{}", document.label()),
				document.label(),
				Ring::Neutral,
				9.,
				move |t, _, cx| {
					t.about_document = if t.about_document == Some(document) { None } else { Some(document) };
					cx.notify();
				},
				window,
				cx,
			)
			.aria_toggled(if pressed { gpui::Toggled::True } else { gpui::Toggled::False }));
	}
	let mut body = div().flex().flex_col().gap(px(8.)).px(px(10.)).xs().child(div().text_color(c(BRIGHT)).child(css_text(format!("Skidbladnir {}{label}, distributed under {license}.", app_version())))).child(div().text_color(c(DIM)).child(css_text(if HEIC_X265 { "This edition encodes HEIC with x265, which is GPL-licensed, so the build as a whole is under the GNU GPL v3 or later. Its complete source is attached to every release." } else { "This edition encodes HEIC with Kvazaar. A GPL edition, with x265, is published beside it." }))).child(div().text_color(c(DIM)).child(css_text(this.backend_version.clone()))).child(buttons);
	if let Some(document) = shown {
		// `<pre>`: the licence's own line breaks kept, long lines wrapped, in its own scroller.
		body = body.child(div().id(ElementId::Name(format!("about-text-{}", document.label()).into())).role(gpui::Role::Document).aria_label(document.label()).max_h(px(384.)).overflow_y_scroll().rounded(px(12.)).bg(c(FIELD)).p(px(12.)).text_size(px(11.)).line_height(px(17.875)).text_color(c(FG)).child(document.text()));
	}
	this.panel("About Skidbladnir", Some(close), vec![body.into_any_element()]).into_any_element()
}
