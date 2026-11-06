//! The `sqb build` completion footer.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::build_progress::main::format_build_footer::format_build_footer as footer;
use sqlbuild_core::build_progress::models::{BuildResultText, PythonNodeText};
use sqlbuild_core::cli_text::models::CliStyle;

use crate::bindings::_helpers::build_progress::result_text::{build_text, python_node_text};

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

/// The build footer, counting `checks` (Python check results) with the Python node results.
pub(crate) fn format_build_footer_with(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_build_footer, module)?)
}
