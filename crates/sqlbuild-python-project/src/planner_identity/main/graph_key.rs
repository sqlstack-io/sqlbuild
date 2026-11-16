//! The graph key of a `CompiledObjectKey`-like value: its resource type text and name.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_identity::models::GraphKey;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The graph key of a `CompiledObjectKey`-like value: its resource type text and name.
pub fn graph_key(key: &Bound<'_, PyAny>) -> PyResult<GraphKey> {
    Ok(GraphKey {
        kind: native_field(key, "resource_type")?.str()?.extract()?,
        name: native_field(key, "name")?.extract()?,
    })
}
