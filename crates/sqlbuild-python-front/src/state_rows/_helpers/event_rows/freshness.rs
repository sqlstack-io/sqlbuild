//! Source freshness data versions, their hashes, skip equivalence and age policy.

use crate::state_rows::main::event_rows::in_utc::in_utc;
use crate::state_rows::main::event_rows::offset_aware::offset_aware;
use crate::state_rows::main::event_rows::presumed_utc::presumed_utc;
use sqlbuild_python_base::records::main::construct::exception_from::exception_from;
use std::cell::RefCell;

use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_state::freshness::errors::FreshnessError;
use sqlbuild_state::freshness::models::{ComparableRecord, FreshnessEventIdentity};
use sqlbuild_state::freshness::types::TimestampParser;

use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::main::values::local_date_time::local_date_time;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

pub(crate) const TIMESTAMP_KIND: &str = "timestamp";
pub(crate) const INTEGER_KIND: &str = "integer";
pub(crate) const STRING_KIND: &str = "string";

/// Normalize timestamp offsets and precision before computing a persisted observation identity.
pub(crate) fn record_event_id(record: &Bound<'_, PyAny>) -> PyResult<String> {
    let observed_at = presumed_utc(&native_field(record, "observed_at")?)?;
    let observed_at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
        observed_at.date.year,
        observed_at.date.month,
        observed_at.date.day,
        observed_at.hour,
        observed_at.minute,
        observed_at.second,
        observed_at.microsecond,
    );
    Ok(sqlbuild_state::freshness::main::event_id::event_id(
        &FreshnessEventIdentity {
            source_name: &native_field(record, "source_name")?.extract::<String>()?,
            target_database: native_field(record, "target_database")?
                .extract::<Option<String>>()?
                .as_deref(),
            target_schema: native_field(record, "target_schema")?
                .extract::<Option<String>>()?
                .as_deref(),
            target_name: native_field(record, "target_name")?
                .extract::<Option<String>>()?
                .as_deref(),
            run_id: &native_field(record, "run_id")?.extract::<String>()?,
            strategy: &native_field(record, "strategy")?.extract::<String>()?,
            value_kind: &native_field(record, "value_kind")?.extract::<String>()?,
            data_version_hash: &native_field(record, "data_version_hash")?.extract::<String>()?,
            observed_at: &observed_at,
        },
    ))
}

pub(crate) fn raised(py: Python<'_>, class: NativeAttr, message: String) -> PyErr {
    exception_from(native_attr(py, class), &message, (None, None))
}

pub(crate) fn python_error(py: Python<'_>, error: FreshnessError) -> PyErr {
    match error {
        FreshnessError::Observation(message) => {
            raised(py, NativeAttr::SourceFreshnessObservationError, message)
        }
        FreshnessError::Input(message) => {
            raised(py, NativeAttr::SourceFreshnessInputError, message)
        }
        FreshnessError::Overflow(message) => PyOverflowError::new_err(message),
    }
}

pub(crate) fn observation_error(py: Python<'_>, message: &str) -> PyErr {
    raised(
        py,
        NativeAttr::SourceFreshnessObservationError,
        message.to_owned(),
    )
}

/// Stored timestamp text parsed with `datetime.fromisoformat`, remembering a Python failure.
pub(crate) struct StateTimestamps<'py> {
    pub(crate) py: Python<'py>,
    pub(crate) failure: RefCell<Option<PyErr>>,
}

impl StateTimestamps<'_> {
    fn parsed(&self, text: &str) -> PyResult<LocalDateTime> {
        let parsed =
            imported(self.py, DATETIME_MODULE, "datetime")?.call_method1("fromisoformat", (text,));
        match parsed {
            Ok(value) => presumed_utc(&value),
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(self.py) => {
                let wrapped: PyErr = observation_error(
                    self.py,
                    &format!(
                        "Timestamp source freshness state value is not valid ISO datetime: {text}"
                    ),
                );
                wrapped.set_cause(self.py, Some(error));
                Err(wrapped)
            }
            Err(error) => Err(error),
        }
    }
}

impl TimestampParser for StateTimestamps<'_> {
    fn utc_timestamp(&self, text: &str) -> Result<LocalDateTime, FreshnessError> {
        self.parsed(text).map_err(|error| {
            let message: String = error.to_string();
            *self.failure.borrow_mut() = Some(error);
            FreshnessError::Observation(message)
        })
    }
}

pub(crate) fn comparable(record: &Bound<'_, PyAny>) -> PyResult<ComparableRecord> {
    Ok(ComparableRecord {
        data_version_hash: native_field(record, "data_version_hash")?.extract()?,
        value_kind: native_field(record, "value_kind")?.extract()?,
        data_version: native_field(record, "data_version")?.extract()?,
    })
}

/// A `datetime` required to be aware, in UTC.
pub(crate) fn aware_utc(value: &Bound<'_, PyAny>, field_name: &str) -> PyResult<LocalDateTime> {
    if !offset_aware(value)? {
        return Err(observation_error(
            value.py(),
            &format!("source freshness {field_name} must be timezone-aware"),
        ));
    }
    local_date_time(&in_utc(value)?)
}
