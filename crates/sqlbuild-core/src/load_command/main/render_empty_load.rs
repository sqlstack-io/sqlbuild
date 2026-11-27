//! The output of a load in a project that loads nothing.

use crate::cli_text::models::CliStyle;

/// A note that the project declares no load or `@loader`.
#[must_use]
pub fn render_empty_load(style: CliStyle) -> String {
    style.muted("  Nothing to load: the project declares no load or @loader.")
}
