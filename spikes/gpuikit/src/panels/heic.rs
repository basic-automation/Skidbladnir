//! Placeholder until the panel is ported.

use gpui::{AnyElement, Context, IntoElement, Window, div};

use crate::app::Skid;

pub fn render(_: &mut Skid, _: &mut Window, _: &mut Context<Skid>) -> AnyElement {
	div().into_any_element()
}
