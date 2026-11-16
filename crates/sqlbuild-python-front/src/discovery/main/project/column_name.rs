//! Column name.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Column name.
pub fn column_name(column: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(column, "name")?.extract()
}
