//! Register the run lint functions and classes.

use crate::rules::_helpers::lint::run_lint::collect_lint_project_files;
use crate::rules::_helpers::lint::run_lint::run_lint;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the run lint functions and classes.
pub fn register_run_lint(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_lint, module)?)?;
    module.add_function(wrap_pyfunction!(collect_lint_project_files, module)?)?;
    Ok(())
}
