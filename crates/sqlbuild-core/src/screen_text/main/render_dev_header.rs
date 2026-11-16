//! The first line of the `sqb dev` screen.

use crate::screen_text::_helpers::rows::{render_row, span};
use crate::screen_text::constants::{PRODUCT, SEPARATOR};
use crate::screen_text::models::{DevHeader, ScreenFacts, Tone};

/// `sqlbuild dev  v0.1.0 · shop · target dev`.
#[must_use]
pub fn render_dev_header(header: &DevHeader, facts: &ScreenFacts) -> String {
    render_row(
        vec![
            span(PRODUCT, Tone::Strong),
            span(
                format!(
                    "  v{}{SEPARATOR}{}{SEPARATOR}target {}",
                    header.version, header.project, header.target
                ),
                Tone::Muted,
            ),
        ],
        facts,
    )
}
