//! Hash the stable source freshness identity used as a graph input.

use pyo3::prelude::PyResult;
use pyo3::pyfunction;
use sqlbuild_state::freshness::main::data_version_hash::data_version_hash;

/// Hash the stable source freshness identity used as a graph input.
#[pyfunction(name = "source_freshness_data_version_hash")]
#[pyo3(signature = (*, source_name, strategy, value_kind, data_version))]
pub fn version_hash(
    source_name: &str,
    strategy: &str,
    value_kind: &str,
    data_version: &str,
) -> PyResult<String> {
    data_version_hash((source_name, strategy), value_kind, data_version)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(format!("{error:?}")))
}
