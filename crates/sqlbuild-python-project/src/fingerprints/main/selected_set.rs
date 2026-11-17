//! The `FingerprintSet` of `latest` fingerprints, narrowed to `node_names` plus unfiltered node types

use crate::fingerprints::_helpers::latest_rows::borrowed;
use crate::fingerprints::_helpers::latest_rows::identity_of;
use crate::fingerprints::_helpers::latest_rows::indexed_set;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_analysis::fingerprint_storage::main::selected_positions::selected_positions;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// `latest` narrowed to `node_names` as a filtered latest read would be; `None` keeps every fingerprint.
pub fn selected_set<'py>(
    py: Python<'py>,
    schema: &str,
    latest: Vec<Bound<'py, PyAny>>,
    (node_names, filtered_node_types): (Option<Vec<String>>, Vec<String>),
) -> PyResult<Bound<'py, PyAny>> {
    let Some(node_names) = node_names else {
        return indexed_set(py, schema, &latest);
    };
    if node_names.is_empty() && filtered_node_types.is_empty() {
        let kwargs: Bound<'py, PyDict> = PyDict::new(py);
        kwargs.set_item("schema", schema)?;
        kwargs.set_item("fingerprints", PyDict::new(py))?;
        return new_record(py, RecordId::FingerprintSet, (), Some(&kwargs));
    }
    let identities: Vec<(String, String)> =
        latest.iter().map(identity_of).collect::<PyResult<_>>()?;
    let selected: Vec<Bound<'py, PyAny>> =
        selected_positions(&borrowed(&identities), &node_names, &filtered_node_types)
            .into_iter()
            .filter_map(|position| latest.get(position).cloned())
            .collect();
    indexed_set(py, schema, &selected)
}
