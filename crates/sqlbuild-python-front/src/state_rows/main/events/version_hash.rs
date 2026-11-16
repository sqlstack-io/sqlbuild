//! Hash the stable source freshness identity used as a graph input.

use crate::state_rows::_helpers::events::freshness;
use pyo3::prelude::PyResult;

/// Hash the stable source freshness identity used as a graph input.
#[inline]
pub fn version_hash(
    source_name: &str,
    strategy: &str,
    value_kind: &str,
    data_version: &str,
) -> PyResult<String> {
    freshness::version_hash(source_name, strategy, value_kind, data_version)
}
