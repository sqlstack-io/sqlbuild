//! Register the reuse attempt functions and classes.

use crate::compile_command::_helpers::invocation::reuse_attempt::attempt_compile_reuse;
use crate::compile_command::constants::EXCLUDED_ROOT_ATTRIBUTE;
use crate::compile_command::constants::EXCLUDED_ROOT_DIRECTORIES;
use crate::compile_command::constants::RACY_WINDOW_ATTRIBUTE;
use crate::compile_command::constants::RACY_WINDOW_NS;
use crate::compile_command::main::invocation::run_compile::run_compile;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyFrozenSet;
use pyo3::wrap_pyfunction;

/// Register the reuse attempt functions and classes.
pub fn register_reuse_attempt(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    module.add(RACY_WINDOW_ATTRIBUTE, RACY_WINDOW_NS)?;
    module.add(
        EXCLUDED_ROOT_ATTRIBUTE,
        PyFrozenSet::new(py, EXCLUDED_ROOT_DIRECTORIES)?,
    )?;
    module.add_function(wrap_pyfunction!(attempt_compile_reuse, module)?)?;
    module.add_function(wrap_pyfunction!(run_compile, module)?)
}
