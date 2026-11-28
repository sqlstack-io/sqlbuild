//! The dim line after the rows of a `--run-on local` run that counts what could not run locally.

use crate::cli_text::models::CliStyle;
use crate::run_output::_helpers::causes::cause_list;
use crate::run_output::models::HeaderCause;

/// A blank line, then `  Can't run locally (N): <causes>; see below`.
#[must_use]
pub fn render_cant_run_rollup(not_run: usize, causes: &[HeaderCause], style: CliStyle) -> String {
    format!(
        "\n{}\n",
        style.muted(&format!(
            "  Can't run locally ({not_run}): {}; see below",
            cause_list(causes)
        ))
    )
}
