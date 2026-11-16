//! `select_latest_fingerprints`: an unfiltered latest read narrowed as a filtered read would be.

use crate::fingerprints::_helpers::latest_rows::borrowed;
use crate::fingerprints::_helpers::latest_rows::identity_of;
use crate::fingerprints::_helpers::latest_rows::indexed_set;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::fingerprint_storage::main::selected_positions::selected_positions;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_values::mapping_values;

/// `select_latest_fingerprints`: an unfiltered latest read narrowed as a filtered read would be.
#[pyfunction]
#[pyo3(signature = (*, fingerprint_set, node_names, filtered_node_types = Vec::new()))]
pub fn select_latest_fingerprints<'py>(
    py: Python<'py>,
    fingerprint_set: &Bound<'py, PyAny>,
    node_names: Vec<String>,
    filtered_node_types: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let schema: String = native_field(fingerprint_set, "schema")?.extract()?;
    if node_names.is_empty() && filtered_node_types.is_empty() {
        return indexed_set(py, &schema, &[]);
    }
    let by_identity: Bound<'py, PyAny> = native_field(fingerprint_set, "fingerprints_by_identity")?;
    let latest: Vec<Bound<'py, PyAny>> = if by_identity.is_truthy()? {
        mapping_values(&by_identity)?
            .try_iter()?
            .collect::<PyResult<_>>()?
    } else {
        Vec::new()
    };
    let identities: Vec<(String, String)> =
        latest.iter().map(identity_of).collect::<PyResult<_>>()?;
    let positions: Vec<usize> =
        selected_positions(&borrowed(&identities), &node_names, &filtered_node_types);
    let selected: Vec<Bound<'py, PyAny>> = positions
        .into_iter()
        .map(|position| latest[position].clone())
        .collect();
    indexed_set(py, &schema, &selected)
}
