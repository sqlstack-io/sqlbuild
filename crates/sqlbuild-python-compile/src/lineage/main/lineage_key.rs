//! A Python `CompiledObjectKey` as a lineage key.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::lineage_command::models::LineageKey;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A Python `CompiledObjectKey` as a lineage key.
pub fn lineage_key(key: &Bound<'_, PyAny>) -> PyResult<LineageKey> {
    Ok(LineageKey {
        resource_type: native_field(key, "resource_type")?.str()?.extract()?,
        name: native_field(key, "name")?.extract()?,
    })
}
