//! Append canonical integration-result records for a parent integration, one per claimed terminal.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

pub(crate) const BUILD_OPTIONS: (&[&str], &[&str]) = (&["result", "plan"], &["command"]);

/// A Python context manager entered for the duration of `body`.
pub(crate) fn within<T>(
    manager: &Bound<'_, PyAny>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    manager.call_method0("__enter__")?;
    let outcome: PyResult<T> = body();
    let none = manager.py().None();
    manager.call_method1("__exit__", (&none, &none, &none))?;
    outcome
}

pub(crate) fn write(stream: &Bound<'_, PyAny>, line: &str) -> PyResult<()> {
    stream.call_method1("write", (line,))?;
    stream.call_method0("flush")?;
    Ok(())
}
