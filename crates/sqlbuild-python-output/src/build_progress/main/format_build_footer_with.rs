//! The build footer, counting `checks` (Python check results) with the Python node results.

use crate::build_progress::_helpers::result_text::python_node_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::build_progress::main::format_build_footer::format_build_footer as footer;
use sqlbuild_core::build_progress::models::{BuildResultText, PythonNodeText};
use sqlbuild_core::cli_text::models::CliStyle;

/// The build footer, counting `checks` (Python check results) with the Python node results.
pub fn format_build_footer_with(
    result: &BuildResultText,
    (elapsed, use_color): (f64, bool),
    python_node_results: Option<&Bound<'_, PyAny>>,
    checks: Vec<PythonNodeText>,
) -> PyResult<String> {
    let mut python: Vec<PythonNodeText> = Vec::new();
    if let Some(results) = python_node_results {
        for node in results.try_iter()? {
            python.push(python_node_text(&node?)?);
        }
    }
    python.extend(checks);
    Ok(footer(result, &python, elapsed, CliStyle { use_color }))
}
