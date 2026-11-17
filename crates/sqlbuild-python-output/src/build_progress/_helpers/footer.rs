//! The `sqb build` completion footer.

use crate::build_progress::main::format_build_footer_with::format_build_footer_with;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

use crate::build_progress::_helpers::result_text::build_text;

/// Render the build completion line, warnings, failures and skipped rollup.
#[pyfunction]
#[pyo3(signature = (*, result, elapsed, use_color, python_node_results = None))]
pub(crate) fn format_build_footer(
    result: &Bound<'_, PyAny>,
    elapsed: f64,
    use_color: bool,
    python_node_results: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    format_build_footer_with(
        &build_text(result)?,
        (elapsed, use_color),
        python_node_results,
        Vec::new(),
    )
}
