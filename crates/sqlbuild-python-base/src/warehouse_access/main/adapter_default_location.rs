//! The adapter's own default database or schema.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The adapter's `default_database` or `default_schema`, named by `method`.
pub fn adapter_default_location<'py>(
    adapter: &Bound<'py, PyAny>,
    method: &str,
) -> PyResult<Bound<'py, PyAny>> {
    adapter.call_method0(method)
}
