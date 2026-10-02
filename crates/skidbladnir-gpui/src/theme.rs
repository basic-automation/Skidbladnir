//! Paleday, as `frontend/assets/css/main.css` defines it, and the type scale the old window
//! uses. Every number here was read from the 1.0.0 frontend's computed styles.

use gpui::{FontWeight, Hsla, Rgba, Styled, px, rgb};

pub const BG: u32 = 0x00d8_d8d0;
pub const FIELD: u32 = 0x00c9_c9bf;
pub const RULE: u32 = 0x00ab_ab9c;
pub const MUTED: u32 = 0x007c_7c67;
pub const DIM: u32 = 0x005b_5b4b;
pub const FG: u32 = 0x002b_2b22;
pub const BRIGHT: u32 = 0x000c_0a09;
pub const ACCENT: u32 = 0x005e_a500;
pub const ACCENT_TEXT: u32 = 0x003c_6a00;
pub const BROWN: u32 = 0x0073_3e0a;
pub const VIOLET: u32 = 0x007f_22fe;
pub const ON_VIOLET: u32 = 0x00f6_f6f1;
pub const WARNING: u32 = 0x008a_5a00;
pub const ERROR: u32 = 0x00b3_261e;

/// The family baked from `@fontsource-variable/fira-code` at the two weights the window
/// uses (assets/fonts). The webview interpolates the variable font; gpui gets static cuts.
pub const FONT: &str = "Fira Code";

/// A Paleday colour.
#[must_use]
pub fn c(hex: u32) -> Hsla {
	rgb(hex).into()
}

/// A Paleday colour at an opacity, as Tailwind's `bg-paleday-field/70`.
#[must_use]
pub fn ca(hex: u32, alpha: f32) -> Hsla {
	let mut colour: Rgba = rgb(hex);
	colour.a = alpha;
	colour.into()
}

/// The body weight (`font-weight: 450`).
pub const REGULAR: FontWeight = FontWeight(450.0);
/// `font-medium`, which Nuxt UI's buttons use.
pub const MEDIUM: FontWeight = FontWeight(500.0);
/// `font-semibold`.
pub const SEMIBOLD: FontWeight = FontWeight(600.0);

/// Tailwind's type steps, with the line heights the old window computes.
pub trait Type: Styled + Sized {
	/// `text-xs`: 12px. Tailwind's line height for it is `calc(1 / 0.75)`, 15.99996px,
	/// which `WebKitGTK` truncates to a 15px line box. The window on Linux is `WebKitGTK`, so
	/// that is the line box to match, not the 16px a Chromium engine lays out.
	fn xs(self) -> Self {
		self.text_size(px(12.)).line_height(px(15.))
	}
	/// `text-sm`: 14px, on `calc(1.25 / 0.875)`, which `WebKitGTK` lays out as 19px.
	fn sm(self) -> Self {
		self.text_size(px(14.)).line_height(px(19.))
	}
	/// `text-[11px]`, which inherits the body's 1.5 line height: 16.5px, a 16px box.
	fn px11(self) -> Self {
		self.text_size(px(11.)).line_height(px(16.))
	}
	/// `text-[10px]`, likewise.
	fn px10(self) -> Self {
		self.text_size(px(10.)).line_height(px(15.))
	}
	fn semibold(self) -> Self {
		self.font_weight(SEMIBOLD)
	}
	fn medium(self) -> Self {
		self.font_weight(MEDIUM)
	}
	fn regular(self) -> Self {
		self.font_weight(REGULAR)
	}
}

impl<T: Styled> Type for T {}
