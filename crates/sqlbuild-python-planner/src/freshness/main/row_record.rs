//! Row record.

use crate::freshness::_helpers::support::optional_str;
use crate::freshness::main::presumed_utc_datetime::presumed_utc_datetime;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::constants::DATETIME_MODULE;
use sqlbuild_python_front::state_rows::main::event_rows::date_time_typed::date_time_typed;

/// Row record.
pub fn row_record<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = row.py();
    let raw_observed_at: Bound<'py, PyAny> = row.get_item(9)?;
    let observed_at: Bound<'py, PyAny> = if date_time_typed(&raw_observed_at)? {
        raw_observed_at
    } else {
        imported(py, DATETIME_MODULE, "datetime")?
            .call_method1("fromisoformat", (raw_observed_at.str()?,))?
    };
    let observed_at: Bound<'py, PyAny> = presumed_utc_datetime(&observed_at)?;
    let field = |index: usize| -> PyResult<Bound<'py, PyAny>> {
        Ok(row.get_item(index)?.str()?.into_any())
    };
    let optional =
        |index: usize| -> PyResult<Bound<'py, PyAny>> { optional_str(&row.get_item(index)?) };
    new_record_with(
        py,
        RecordId::SourceFreshnessRecord,
        &[
            ("source_name", field(0)?),
            ("target_database", optional(1)?),
            ("target_schema", optional(2)?),
            ("target_name", optional(3)?),
            ("run_id", field(4)?),
            ("strategy", field(5)?),
            ("value_kind", field(6)?),
            ("data_version", optional(7)?),
            ("data_version_hash", field(8)?),
            ("observed_at", observed_at),
        ],
    )
}
