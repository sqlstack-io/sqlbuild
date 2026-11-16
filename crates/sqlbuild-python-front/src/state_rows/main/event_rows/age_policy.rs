//! Optional timestamp age policy for one source freshness observation.

use crate::state_rows::_helpers::event_rows::freshness::aware_utc;
use crate::state_rows::_helpers::event_rows::freshness::python_error;
use crate::state_rows::main::event_rows::date_time_typed::date_time_typed;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_state::freshness::main::age_status::age_status;
use sqlbuild_state::freshness::models::AgeStatus;

/// Optional timestamp age policy for one source freshness observation.
#[pyfunction(name = "evaluate_source_freshness_age_policy")]
#[pyo3(signature = (*, policy, data_version, observed_at))]
pub fn age_policy<'py>(
    policy: &Bound<'py, PyAny>,
    data_version: &Bound<'py, PyAny>,
    observed_at: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = policy.py();
    if policy.is_none() {
        return Ok(None);
    }
    let statuses: Bound<'py, PyAny> = record_class(py, RecordId::SourceFreshnessAgeStatus)?;
    if !date_time_typed(data_version)? {
        return native_field(&statuses, "UNKNOWN").map(Some);
    }
    let version: LocalDateTime = aware_utc(data_version, "data_version")?;
    let observed: LocalDateTime = aware_utc(observed_at, "observed_at")?;
    let error_after: Option<String> = native_field(policy, "error_after")?.extract()?;
    let warn_after: Option<String> = native_field(policy, "warn_after")?.extract()?;
    let status: AgeStatus = age_status(
        (&version, &observed),
        error_after.as_deref(),
        warn_after.as_deref(),
    )
    .map_err(|error| python_error(py, error))?;
    let member: &str = match status {
        AgeStatus::Pass => "PASS",
        AgeStatus::Warn => "WARN",
        AgeStatus::Error => "ERROR",
    };
    enum_member(py, RecordId::SourceFreshnessAgeStatus, member).map(Some)
}
