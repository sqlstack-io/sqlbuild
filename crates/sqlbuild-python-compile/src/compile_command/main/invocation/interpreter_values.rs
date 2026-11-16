//! `sys.version`, `sys.executable`, `sys.platform` and `str(sys.implementation.cache_tag)`.

use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `sys.version`, `sys.executable`, `sys.platform` and `str(sys.implementation.cache_tag)`.
pub fn interpreter_values(py: Python<'_>) -> PyResult<[String; 4]> {
    let sys: Bound<'_, PyModule> = py.import("sys")?;
    Ok([
        native_field(&sys, "version")?.extract()?,
        native_field(&sys, "executable")?.extract()?,
        native_field(&sys, "platform")?.extract()?,
        native_field(&sys, "implementation")?
            .getattr("cache_tag")?
            .str()?
            .extract()?,
    ])
}
