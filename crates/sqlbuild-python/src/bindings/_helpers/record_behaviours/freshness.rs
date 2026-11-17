//! Validation and identity of the natively hosted source freshness state record.

use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::PyDict;

use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_planner::freshness::main::require_aware_utc_datetime::require_aware_utc_datetime;

/// `SourceFreshnessRecord.observed_at` position.
const OBSERVED_AT_FIELD: usize = 9;
const IDENTITY_FIELDS: [&str; 4] = [
    "source_name",
    "target_database",
    "target_schema",
    "target_name",
];

/// `SourceFreshnessRecord`: its observed time made aware UTC, a naive time an error.
pub(crate) fn finish_freshness_record(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let observed_at: Bound<'_, PyAny> =
        require_aware_utc_datetime(values[OBSERVED_AT_FIELD].bind(py), "observed_at")?;
    values[OBSERVED_AT_FIELD] = observed_at.unbind();
    Ok(values)
}

/// `SourceFreshnessRecord.identity`: the source and target the observation belongs to.
pub(crate) fn freshness_record_identity(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    for name in IDENTITY_FIELDS {
        pyo3::types::PyDictMethods::set_item(&keywords, name, native_field(instance, name)?)?;
    }
    Ok(new_record(py, RecordId::SourceFreshnessIdentity, (), Some(&keywords))?.unbind())
}
