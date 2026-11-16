//! `print(*values, file=sys.<stream>)`.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::keywords::keywords;

/// `print(*values, file=sys.<stream>)`.
pub fn print_to<'py>(
    py: Python<'py>,
    stream: &str,
    text: impl pyo3::IntoPyObject<'py>,
) -> PyResult<()> {
    let keywords: Bound<'_, PyDict> =
        keywords(py, &[("file", py.import("sys")?.getattr(stream)?)])?;
    let _ = py
        .import("builtins")?
        .getattr("print")?
        .call((text,), Some(&keywords))?;
    Ok(())
}
