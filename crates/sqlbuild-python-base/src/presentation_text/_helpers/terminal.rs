//! Terminal colour capability of the active output stream.

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};

const NO_COLOR_VARIABLE: &str = "NO_COLOR";

/// Whether `sys.stdout` is a terminal and `NO_COLOR` is unset.
pub(crate) fn stdout_supports_color(py: Python<'_>) -> PyResult<bool> {
    if std::env::var_os(NO_COLOR_VARIABLE).is_some() {
        return Ok(false);
    }
    stdout_is_tty(py)
}

/// Whether `sys.stdout` is a terminal.
pub(crate) fn stdout_is_tty(py: Python<'_>) -> PyResult<bool> {
    let stdout = py.import("sys")?.getattr("stdout")?;
    if !stdout.hasattr("isatty")? {
        return Ok(false);
    }
    stdout.call_method0("isatty")?.is_truthy()
}

/// Return whether the active output terminal supports color.
#[pyfunction(name = "supports_color")]
fn color_support(py: Python<'_>) -> PyResult<bool> {
    stdout_supports_color(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(color_support, module)?)
}
