//! The graph key of a `CompiledObjectKey`-like value: its resource type text and name.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::planner_identity::models::GraphKey;

/// The graph key of a `CompiledObjectKey`-like value: its resource type text and name.
#[inline]
pub fn graph_key(key: &Bound<'_, PyAny>) -> PyResult<GraphKey> {
    hashing::graph_key(key)
}
