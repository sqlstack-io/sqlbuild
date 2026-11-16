//! Register the lint expansion functions and classes.

use crate::rules::_helpers::lint::lint_expansion::build_lint_expansion_context;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the lint expansion functions and classes.
pub fn register_lint_expansion(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_lint_expansion_context, module)?)
}
