//! Source freshness data versions, their hashes, skip equivalence and age policy.

use std::cell::RefCell;

use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cursor::algebra::models::LocalDateTime;
use sqlbuild_state::freshness::errors::FreshnessError;
use sqlbuild_state::freshness::main::age_status::age_status;
use sqlbuild_state::freshness::main::data_version_hash::data_version_hash;
use sqlbuild_state::freshness::main::normalized_data_version::normalized_data_version;
use sqlbuild_state::freshness::main::records_equivalent::records_equivalent;
use sqlbuild_state::freshness::models::{
    AgeStatus, ComparableRecord, DataVersion, FreshnessEventIdentity,
};
use sqlbuild_state::freshness::types::TimestampParser;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::cursor::values::{DATETIME_MODULE, local_date_time};
use crate::bindings::_helpers::records::construct::{enum_member, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{NativeAttr, RecordId};
use crate::bindings::_helpers::records::registry;
use crate::bindings::_helpers::state_rows::timestamps::{
    date_time_typed, in_utc, offset_aware, presumed_utc,
};

const TIMESTAMP_KIND: &str = "timestamp";
const INTEGER_KIND: &str = "integer";
const STRING_KIND: &str = "string";

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

fn raised(py: Python<'_>, class: NativeAttr, message: String) -> PyErr {
    match registry::native(py, class).and_then(|class| class.call1((message,))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn python_error(py: Python<'_>, error: FreshnessError) -> PyErr {
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

/// Hash the stable source freshness identity used as a graph input.
#[pyfunction(name = "source_freshness_data_version_hash")]
#[pyo3(signature = (*, source_name, strategy, value_kind, data_version))]
pub(crate) fn version_hash(
    source_name: &str,
    strategy: &str,
    value_kind: &str,
    data_version: &str,
) -> PyResult<String> {
    data_version_hash((source_name, strategy), value_kind, data_version)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(format!("{error:?}")))
}

fn observation_error(py: Python<'_>, message: &str) -> PyErr {
    raised(
        py,
        NativeAttr::SourceFreshnessObservationError,
        message.to_owned(),
    )
}

/// A source freshness value normalized for stable state storage.
#[pyfunction(name = "normalize_source_freshness_data_version")]
#[pyo3(signature = (*, value, value_kind))]
pub(crate) fn normalize<'py>(
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

/// Stored timestamp text parsed with `datetime.fromisoformat`, remembering a Python failure.
struct StateTimestamps<'py> {
    py: Python<'py>,
    failure: RefCell<Option<PyErr>>,
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

fn comparable(record: &Bound<'_, PyAny>) -> PyResult<ComparableRecord> {
    Ok(ComparableRecord {
        data_version_hash: native_field(record, "data_version_hash")?.extract()?,
        value_kind: native_field(record, "value_kind")?.extract()?,
        data_version: native_field(record, "data_version")?.extract()?,
    })
}

/// Whether two source freshness records are equivalent for skip decisions.
#[pyfunction(name = "source_freshness_records_equivalent")]
#[pyo3(signature = (*, previous_record, current_record, lag_tolerance = None))]
pub(crate) fn equivalent(
    previous_record: &Bound<'_, PyAny>,
    current_record: &Bound<'_, PyAny>,
    lag_tolerance: Option<&str>,
) -> PyResult<bool> {
    let py: Python<'_> = previous_record.py();
    let parser = StateTimestamps {
        py,
        failure: RefCell::new(None),
    };
    let records = (&comparable(previous_record)?, &comparable(current_record)?);
    records_equivalent(records, lag_tolerance, &parser).map_err(|error| {
        parser
            .failure
            .borrow_mut()
            .take()
            .unwrap_or_else(|| python_error(py, error))
    })
}

/// A `datetime` required to be aware, in UTC.
fn aware_utc(value: &Bound<'_, PyAny>, field_name: &str) -> PyResult<LocalDateTime> {
    if !offset_aware(value)? {
        return Err(observation_error(
            value.py(),
            &format!("source freshness {field_name} must be timezone-aware"),
        ));
    }
    local_date_time(&in_utc(value)?)
}

/// Optional timestamp age policy for one source freshness observation.
#[pyfunction(name = "evaluate_source_freshness_age_policy")]
#[pyo3(signature = (*, policy, data_version, observed_at))]
pub(crate) fn age_policy<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version_hash, module)?)?;
    module.add_function(wrap_pyfunction!(normalize, module)?)?;
    module.add_function(wrap_pyfunction!(equivalent, module)?)?;
    module.add_function(wrap_pyfunction!(age_policy, module)?)?;
    Ok(())
}
