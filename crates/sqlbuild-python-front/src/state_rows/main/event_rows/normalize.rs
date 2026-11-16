//! A source freshness value normalized for stable state storage.

use crate::cursor::main::values::local_date_time::local_date_time;
use crate::state_rows::_helpers::event_rows::freshness::INTEGER_KIND;
use crate::state_rows::_helpers::event_rows::freshness::STRING_KIND;
use crate::state_rows::_helpers::event_rows::freshness::TIMESTAMP_KIND;
use crate::state_rows::_helpers::event_rows::freshness::observation_error;
use crate::state_rows::_helpers::event_rows::freshness::raised;
use crate::state_rows::main::event_rows::date_time_typed::date_time_typed;
use crate::state_rows::main::event_rows::in_utc::in_utc;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyInt, PyString};
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_state::freshness::main::normalized_data_version::normalized_data_version;
use sqlbuild_state::freshness::models::DataVersion;

/// A source freshness value normalized for stable state storage.
#[pyfunction(name = "normalize_source_freshness_data_version")]
#[pyo3(signature = (*, value, value_kind))]
pub fn normalize<'py>(
    value: &Bound<'py, PyAny>,
    value_kind: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if value_kind.eq(TIMESTAMP_KIND)? {
        if !date_time_typed(value)? {
            return Err(observation_error(
                py,
                "Timestamp source freshness values must be datetime values",
            ));
        }
        let aware: bool = !native_field(value, "tzinfo")?.is_none();
        let local: LocalDateTime = if aware {
            local_date_time(&in_utc(value)?)?
        } else {
            local_date_time(value)?
        };
        let text: String = normalized_data_version(&DataVersion::Timestamp { local, utc: aware });
        return Ok(PyString::new(py, &text).into_any());
    }
    if value_kind.eq(INTEGER_KIND)? {
        if value.is_instance_of::<PyBool>() || !value.is_instance_of::<PyInt>() {
            return Err(observation_error(
                py,
                "Integer source freshness values must be integer values",
            ));
        }
        return Ok(value.str()?.into_any());
    }
    if value_kind.eq(STRING_KIND)? {
        if !value.is_instance_of::<PyString>() {
            return Err(observation_error(
                py,
                "String source freshness values must be string values",
            ));
        }
        return Ok(value.clone());
    }
    Err(raised(
        py,
        NativeAttr::SourceFreshnessObservationError,
        format!(
            "Unsupported source freshness value kind: {}",
            value_kind.str()?
        ),
    ))
}
