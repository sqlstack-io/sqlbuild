//! Python's `inferred_binding_shape` test for keeping authored identifier quoting.

use crate::analysis::constants::CASE_SENSITIVE_BINDING_DIALECTS;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Python's `inferred_binding_shape` test for keeping authored identifier quoting.
pub fn case_sensitive_shapes(
    profile: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if native_field(profile, "quoted_identifiers_ignore_case")?.is_truthy()? {
        return Ok(false);
    }
    if dialect.is_none() {
        return Ok(false);
    }
    let dialect: String = dialect.extract()?;
    Ok(CASE_SENSITIVE_BINDING_DIALECTS.contains(&dialect.as_str()))
}
