//! The execution header block written before command progress.

use crate::cli_text::main::format_surface_header::format_surface_header;
use crate::cli_text::models::CliStyle;
use crate::progress::constants::EXECUTION_TITLE;
use crate::progress::main::format_execution_header::format_execution_header;

/// The styled `Execution` header with its context and the engines that run, followed by a blank line.
#[must_use]
pub fn execution_header_text(
    style: CliStyle,
    (command, target, concurrency): (&str, Option<&str>, i64),
    engines: Option<&str>,
) -> String {
    let mut context: String = format_execution_header(command, target, concurrency);
    if let Some(engines) = engines {
        context.pop();
        context.push_str(&format!(", {engines})"));
    }
    format!(
        "{}\n\n",
        format_surface_header(style, EXECUTION_TITLE, &context)
    )
}
