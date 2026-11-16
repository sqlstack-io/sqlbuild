//! The text layout `sqb query` and `sqb recall query` share: a box when the result fits the output width, records otherwise.

use pyo3::prelude::{PyResult, Python};
use sqlbuild_core::query_command::main::render_query_text::render_query_text;
use sqlbuild_core::query_command::models::{QueryDisplay, QueryLayout};

use sqlbuild_python_base::build_progress::main::terminal::terminal_columns::terminal_columns;
use sqlbuild_python_base::presentation_text::main::stdout_is_tty::stdout_is_tty;

/// The format that always prints one record per row.
pub(crate) const LONG_FORMAT: &str = "long";
/// The width a box may take when stdout is not a terminal.
const PIPED_WIDTH: usize = 120;

/// The terminal's width when stdout is one, else [`PIPED_WIDTH`].
fn output_width(py: Python<'_>) -> PyResult<usize> {
    if !stdout_is_tty(py).unwrap_or(false) {
        return Ok(PIPED_WIDTH);
    }
    terminal_columns(py)
}

/// `display` as text: records for `--format long`, otherwise a box when it fits the output.
pub(crate) fn render_text(
    py: Python<'_>,
    display: &QueryDisplay,
    format: Option<&str>,
) -> PyResult<String> {
    let layout: QueryLayout = if format == Some(LONG_FORMAT) {
        QueryLayout::Long
    } else {
        QueryLayout::Fitted {
            width: output_width(py)?,
        }
    };
    Ok(render_query_text(display, layout))
}
