//! Row fingerprint.

use crate::fingerprints::_helpers::latest_rows::TIMESTAMP_COLUMN;
use crate::fingerprints::_helpers::latest_rows::decoded;
use crate::fingerprints::_helpers::latest_rows::optional_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDateTime, PyDict, PyString};

/// Row fingerprint.
pub fn row_fingerprint<'py>(
    fingerprint_class: &Bound<'py, PyAny>,
    from_iso: &Bound<'py, PyAny>,
    row: &Bound<'py, PyAny>,
    qualified_name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = row.py();
    let raw_ts: Bound<'py, PyAny> = row.get_item(TIMESTAMP_COLUMN)?;
    let ts: Bound<'py, PyAny> = if raw_ts.is_instance_of::<PyDateTime>() {
        raw_ts
    } else {
        from_iso.call1((raw_ts.str()?,))?
    };
    let node_type: Bound<'py, PyString> = row.get_item(0)?.str()?;
    let node_name: Bound<'py, PyString> = row.get_item(1)?.str()?;
    let raw_target_database: Bound<'py, PyAny> = row.get_item(2)?;
    let raw_target_schema: Bound<'py, PyAny> = row.get_item(3)?;
    let raw_target_name: Bound<'py, PyAny> = row.get_item(4)?;
    let definition_storage: Bound<'py, PyString> = row.get_item(9)?.str()?;
    let metadata_json_storage: Bound<'py, PyString> = row.get_item(10)?.str()?;
    let definition: String = decoded(&definition_storage, &node_name, qualified_name)?;
    let metadata_json: String = decoded(&metadata_json_storage, &node_name, qualified_name)?;
    let kwargs: Bound<'py, PyDict> = PyDict::new(py);
    kwargs.set_item("node_type", node_type)?;
    kwargs.set_item("node_name", node_name)?;
    kwargs.set_item("target_database", optional_text(&raw_target_database)?)?;
    kwargs.set_item("target_schema", optional_text(&raw_target_schema)?)?;
    kwargs.set_item("target_name", optional_text(&raw_target_name)?)?;
    kwargs.set_item("run_id", row.get_item(5)?.str()?)?;
    kwargs.set_item("definition_hash", row.get_item(6)?.str()?)?;
    kwargs.set_item("version_hash", row.get_item(7)?.str()?)?;
    kwargs.set_item("schema_fingerprint", row.get_item(8)?.str()?)?;
    kwargs.set_item("definition", definition)?;
    kwargs.set_item("metadata_json", metadata_json)?;
    kwargs.set_item("ts", ts)?;
    fingerprint_class.call((), Some(&kwargs))
}
