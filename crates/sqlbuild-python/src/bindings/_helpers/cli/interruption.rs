//! The `sqb` process's Ctrl+C handling, around the whole command.

use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::cli::termination::terminated;

/// Call `run` as the `sqb` process's command and return its exit code, 130 once Ctrl+C stopped it.
#[pyfunction]
fn run_interruptible(run: &Bound<'_, PyAny>) -> PyResult<i64> {
    sqlbuild_python_base::native_host::main::run_interruptible::run_interruptible(run, terminated)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_interruptible, module)?)
}
