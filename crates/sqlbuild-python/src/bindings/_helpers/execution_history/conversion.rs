//! Execution history models, timestamps, SQL parameters and errors across the Python boundary.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyInt, PyList, PyString, PyStringMethods, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;
use sqlbuild_state::execution_history::constants::ENVELOPE_TEXT_FIELDS;
use sqlbuild_state::execution_history::errors::{HistoryError, HistoryErrorKind};
use sqlbuild_state::execution_history::models::{
    EventEnvelope, EventFilterSpec, FilterTime, ProjectionFact, RunFact, RunFilterSpec, RunRow,
    RunStatus, SqlParam,
};
use sqlbuild_state::execution_history::types::UtcTime;

use crate::bindings::_helpers::cursor::values::{DATETIME_MODULE, local_date_time};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const UTC_SUFFIX: &str = "Z";
const UTC_OFFSET: &str = "+00:00";
const DEFAULT_SCHEMA_VERSION: i64 = 1;
const UTC_OFFSET_SECONDS: f64 = 0.0;

fn exception_class(kind: HistoryErrorKind) -> ExecutorHost {
    match kind {
        HistoryErrorKind::Storage => ExecutorHost::ExecutionHistoryStorageError,
        HistoryErrorKind::IntegrityConflict => ExecutorHost::IntegrityConflictError,
        HistoryErrorKind::InvalidEvent => ExecutorHost::InvalidEventError,
        HistoryErrorKind::InvalidCursor => ExecutorHost::InvalidCursorError,
        HistoryErrorKind::InvalidFilter => ExecutorHost::InvalidFilterError,
        HistoryErrorKind::InvalidLimit => ExecutorHost::InvalidLimitError,
        HistoryErrorKind::UnsupportedSchemaVersion => ExecutorHost::UnsupportedSchemaVersionError,
        HistoryErrorKind::ProjectionConsistency => ExecutorHost::ProjectionConsistencyError,
    }
}

/// The execution history exception for a rejection of `kind`.
pub(crate) fn rejection(py: Python<'_>, kind: HistoryErrorKind, message: &str) -> PyErr {
    host_error(py, exception_class(kind), message)
}

/// An `ExecutionHistoryStorageError`.
pub(crate) fn storage_error(py: Python<'_>, message: &str) -> PyErr {
    rejection(py, HistoryErrorKind::Storage, message)
}

/// An `ExecutionHistoryStorageError` raised `from cause`.
pub(crate) fn storage_error_from(py: Python<'_>, message: &str, cause: PyErr) -> PyErr {
    let error: PyErr = storage_error(py, message);
    error.set_cause(py, Some(cause));
    error
}

/// `result`, with a history rejection raised as its Python exception.
pub(crate) fn checked<T>(py: Python<'_>, result: Result<T, HistoryError>) -> PyResult<T> {
    result.map_err(|error| rejection(py, error.kind, &error.message))
}

/// Whether `error` is an instance of the execution history exception named `name`.
pub(crate) fn history_error_of_kind(
    py: Python<'_>,
    error: &PyErr,
    name: ExecutorHost,
) -> PyResult<bool> {
    Ok(error.is_instance(py, &host_entry(py, name)?))
}

/// `value` when it is an `int` other than `bool` that fits, as `isinstance(value, int)` checks.
pub(crate) fn exact_int(value: &Bound<'_, PyAny>) -> Option<i64> {
    if !value.is_instance_of::<PyInt>() || value.is_instance_of::<PyBool>() {
        return None;
    }
    value.extract().unwrap_or(None)
}

/// `value` as text when it is a `str`.
pub(crate) fn text_of(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    match value.downcast::<PyString>() {
        Ok(text) => Ok(Some(text.to_str()?.to_owned())),
        Err(_) => Ok(None),
    }
}

/// `value` as text, or `None` when it is `None`.
pub(crate) fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// `datetime.UTC`.
pub(crate) fn utc(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "UTC")
}

/// An aware `datetime` as its UTC wall time.
pub(crate) fn utc_time(value: &Bound<'_, PyAny>) -> PyResult<UtcTime> {
    local_date_time(&value.call_method1("astimezone", (utc(value.py())?,))?)
}

/// A UTC `datetime` at `value`.
pub(crate) fn python_time<'py>(py: Python<'py>, value: &UtcTime) -> PyResult<Bound<'py, PyAny>> {
    imported(py, DATETIME_MODULE, "datetime")?.call1((
        value.date.year,
        value.date.month,
        value.date.day,
        value.hour,
        value.minute,
        value.second,
        value.microsecond,
        utc(py)?,
    ))
}

/// `datetime.now(UTC)`.
pub(crate) fn now(py: Python<'_>) -> PyResult<UtcTime> {
    let current = imported(py, DATETIME_MODULE, "datetime")?.call_method1("now", (utc(py)?,))?;
    local_date_time(&current)
}

/// Whether a `datetime`'s `tzinfo` is exactly `UTC`.
pub(crate) fn utc_zone(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(native_field(value, "tzinfo")?.is(&utc(value.py())?))
}

/// `datetime.fromisoformat(text.replace("Z", "+00:00"))`, or `None` on `ValueError`.
pub(crate) fn parsed_iso<'py>(py: Python<'py>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let normalized: String = text.replace(UTC_SUFFIX, UTC_OFFSET);
    match imported(py, DATETIME_MODULE, "datetime")?.call_method1("fromisoformat", (normalized,)) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

fn filter_time(value: &Bound<'_, PyAny>) -> PyResult<Option<FilterTime>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(FilterTime {
        instant: utc_time(value)?,
        is_utc: utc_zone(value)?,
    }))
}

fn texts(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    value
        .try_iter()?
        .map(|item| item.and_then(|item| item.extract()))
        .collect()
}

/// An `EventFilter`'s fields.
pub(crate) fn event_filter_spec(value: &Bound<'_, PyAny>) -> PyResult<EventFilterSpec> {
    let family: Bound<'_, PyAny> = native_field(value, "family")?;
    Ok(EventFilterSpec {
        invocation_id: optional_text(&native_field(value, "invocation_id")?)?,
        run_id: optional_text(&native_field(value, "run_id")?)?,
        producer: optional_text(&native_field(value, "producer")?)?,
        event_types: texts(&native_field(value, "event_types")?)?,
        family: if family.is_none() {
            None
        } else {
            Some(native_field(&family, "value")?.extract()?)
        },
        occurred_at_start: filter_time(&native_field(value, "occurred_at_start")?)?,
        occurred_at_end: filter_time(&native_field(value, "occurred_at_end")?)?,
    })
}

/// A `RunStatus` member's status.
fn run_status(value: &Bound<'_, PyAny>) -> PyResult<RunStatus> {
    let text: String = native_field(value, "value")?.extract()?;
    parsed_status(value.py(), &text)
}

/// The status named by stored `text`, as `RunStatus(text)` accepts it.
pub(crate) fn parsed_status(py: Python<'_>, text: &str) -> PyResult<RunStatus> {
    let known: Option<RunStatus> = [RunStatus::Unknown, RunStatus::Completed, RunStatus::Failed]
        .into_iter()
        .find(|status| status.value() == text);
    if let Some(status) = known {
        return Ok(status);
    }
    host_entry(py, ExecutorHost::RunStatus)?.call1((text,))?;
    Ok(RunStatus::Unknown)
}

/// A `RunFilter`'s fields.
pub(crate) fn run_filter_spec(value: &Bound<'_, PyAny>) -> PyResult<RunFilterSpec> {
    let statuses: Vec<RunStatus> = native_field(value, "statuses")?
        .try_iter()?
        .map(|item| item.and_then(|item| run_status(&item)))
        .collect::<PyResult<_>>()?;
    Ok(RunFilterSpec {
        invocation_id: optional_text(&native_field(value, "invocation_id")?)?,
        statuses,
        created_at_start: filter_time(&native_field(value, "created_at_start")?)?,
        created_at_end: filter_time(&native_field(value, "created_at_end")?)?,
    })
}

/// Whether `event` is a known-schema `LifecycleEvent`.
pub(crate) fn known_event(event: &Bound<'_, PyAny>) -> PyResult<bool> {
    event.is_instance(&host_entry(event.py(), ExecutorHost::LifecycleEvent)?)
}

/// The `event_id` a known event carries or an opaque envelope holds as a string.
pub(crate) fn event_id_of(event: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if known_event(event)? {
        return text_of(&native_field(event, "event_id")?);
    }
    text_of(&raw_field(event, "event_id")?)
}

fn raw_field<'py>(event: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(event, "raw")?.call_method1("get", (name,))
}

/// `lifecycle_event_to_json(event)`.
pub(crate) fn event_json(event: &Bound<'_, PyAny>) -> PyResult<String> {
    host_entry(event.py(), ExecutorHost::LifecycleEventToJson)?
        .call1((event,))?
        .extract()
}

/// `lifecycle_event_from_json(payload)`.
pub(crate) fn event_from_json<'py>(payload: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    host_entry(payload.py(), ExecutorHost::LifecycleEventFromJson)?.call1((payload,))
}

/// An opaque envelope's `occurred_at` when it is ISO text with a zero UTC offset.
fn opaque_occurred_at(event: &Bound<'_, PyAny>) -> PyResult<Option<UtcTime>> {
    let py: Python<'_> = event.py();
    let Some(text) = text_of(&raw_field(event, "occurred_at")?)? else {
        return Ok(None);
    };
    let Some(parsed) = parsed_iso(py, &text)? else {
        return Ok(None);
    };
    let offset: Bound<'_, PyAny> = parsed.call_method0("utcoffset")?;
    if offset.is_none()
        || offset.call_method0("total_seconds")?.extract::<f64>()? != UTC_OFFSET_SECONDS
    {
        return Ok(None);
    }
    Ok(Some(utc_time(&parsed)?))
}

/// The stable envelope fields stored next to an event.
pub(crate) fn envelope_of(event: &Bound<'_, PyAny>) -> PyResult<EventEnvelope> {
    let mut envelope = EventEnvelope::default();
    if known_event(event)? {
        envelope.schema_version = native_field(event, "schema_version")?.extract()?;
        for (slot, name) in envelope.text_fields.iter_mut().zip(ENVELOPE_TEXT_FIELDS) {
            *slot = optional_text(&native_field(event, name)?)?;
        }
        envelope.occurred_at = Some(utc_time(&native_field(event, "occurred_at")?)?);
        return Ok(envelope);
    }
    envelope.schema_version =
        exact_int(&raw_field(event, "schema_version")?).unwrap_or(DEFAULT_SCHEMA_VERSION);
    for (slot, name) in envelope.text_fields.iter_mut().zip(ENVELOPE_TEXT_FIELDS) {
        *slot = text_of(&raw_field(event, name)?)?.filter(|text| !text.is_empty());
    }
    envelope.occurred_at = opaque_occurred_at(event)?;
    Ok(envelope)
}

/// A `StoredEvent` as projection reads it.
pub(crate) fn projection_fact(stored: &Bound<'_, PyAny>) -> PyResult<ProjectionFact> {
    let event: Bound<'_, PyAny> = native_field(stored, "event")?;
    let mut run: Option<RunFact> = None;
    if known_event(&event)? {
        let run_id: Bound<'_, PyAny> = native_field(&event, "run_id")?;
        if !run_id.is_none() {
            run = Some(RunFact {
                run_id: run_id.extract()?,
                invocation_id: native_field(&event, "invocation_id")?.extract()?,
                event_type: native_field(&event, "event_type")?.extract()?,
                occurred_at: utc_time(&native_field(&event, "occurred_at")?)?,
            });
        }
    }
    Ok(ProjectionFact {
        storage_order: native_field(stored, "storage_order")?.extract()?,
        cursor: native_field(stored, "cursor")?.extract()?,
        received_at: utc_time(&native_field(stored, "received_at")?)?,
        run,
    })
}

/// A `RunRecord` as a projected run.
pub(crate) fn run_row(record: &Bound<'_, PyAny>) -> PyResult<RunRow> {
    let optional_time = |name: &str| -> PyResult<Option<UtcTime>> {
        let value: Bound<'_, PyAny> = native_field(record, name)?;
        if value.is_none() {
            return Ok(None);
        }
        utc_time(&value).map(Some)
    };
    Ok(RunRow {
        run_id: native_field(record, "run_id")?.extract()?,
        invocation_id: native_field(record, "invocation_id")?.extract()?,
        created_at: utc_time(&native_field(record, "created_at")?)?,
        status: run_status(&native_field(record, "status")?)?,
        is_complete: native_field(record, "is_complete")?.is_truthy()?,
        last_event_cursor: native_field(record, "last_event_cursor")?.extract()?,
        last_storage_order: native_field(record, "last_storage_order")?.extract()?,
        command: optional_text(&native_field(record, "command")?)?,
        target: optional_text(&native_field(record, "target")?)?,
        environment: optional_text(&native_field(record, "environment")?)?,
        started_at: optional_time("started_at")?,
        ended_at: optional_time("ended_at")?,
    })
}

fn optional_python_time<'py>(
    py: Python<'py>,
    value: Option<&UtcTime>,
) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Some(value) => python_time(py, value),
        None => Ok(py.None().into_bound(py)),
    }
}

/// The `RunRecord` of a projected run.
pub(crate) fn run_record<'py>(py: Python<'py>, run: &RunRow) -> PyResult<Bound<'py, PyAny>> {
    let status = host_entry(py, ExecutorHost::RunStatus)?.call1((run.status.value(),))?;
    call_with(
        &host_entry(py, ExecutorHost::RunRecord)?,
        &[
            ("run_id", run.run_id.clone().into_bound_py_any(py)?),
            (
                "invocation_id",
                run.invocation_id.clone().into_bound_py_any(py)?,
            ),
            ("created_at", python_time(py, &run.created_at)?),
            ("status", status),
            ("is_complete", run.is_complete.into_bound_py_any(py)?),
            (
                "last_event_cursor",
                run.last_event_cursor.clone().into_bound_py_any(py)?,
            ),
            (
                "last_storage_order",
                run.last_storage_order.into_bound_py_any(py)?,
            ),
            ("command", run.command.clone().into_bound_py_any(py)?),
            ("target", run.target.clone().into_bound_py_any(py)?),
            (
                "environment",
                run.environment.clone().into_bound_py_any(py)?,
            ),
            (
                "started_at",
                optional_python_time(py, run.started_at.as_ref())?,
            ),
            ("ended_at", optional_python_time(py, run.ended_at.as_ref())?),
        ],
    )
}

/// A tuple of the `RunRecord`s of projected runs.
pub(crate) fn run_records<'py>(py: Python<'py>, runs: &[RunRow]) -> PyResult<Bound<'py, PyTuple>> {
    let records: Vec<Bound<'py, PyAny>> = runs
        .iter()
        .map(|run| run_record(py, run))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, records)
}

/// The projected runs of an iterable of `RunRecord`s.
pub(crate) fn run_rows(records: &Bound<'_, PyAny>) -> PyResult<Vec<RunRow>> {
    records
        .try_iter()?
        .map(|record| record.and_then(|record| run_row(&record)))
        .collect()
}

/// The projection facts of an iterable of `StoredEvent`s.
pub(crate) fn projection_facts(stored: &Bound<'_, PyAny>) -> PyResult<Vec<ProjectionFact>> {
    stored
        .try_iter()?
        .map(|item| item.and_then(|item| projection_fact(&item)))
        .collect()
}

/// A `StoredEvent` decoded from its durable columns.
pub(crate) fn stored_event<'py>(
    py: Python<'py>,
    (storage_order, cursor): (i64, String),
    received_at: Bound<'py, PyAny>,
    payload: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(py, ExecutorHost::StoredEvent)?,
        &[
            ("storage_order", storage_order.into_bound_py_any(py)?),
            ("cursor", cursor.into_bound_py_any(py)?),
            ("received_at", received_at),
            ("event", event_from_json(payload)?),
        ],
    )
}

/// A SQL parameter as the driver binds it.
pub(crate) fn python_param<'py>(py: Python<'py>, param: &SqlParam) -> PyResult<Bound<'py, PyAny>> {
    match param {
        SqlParam::Null => Ok(py.None().into_bound(py)),
        SqlParam::Text(text) => text.clone().into_bound_py_any(py),
        SqlParam::Integer(value) => value.into_bound_py_any(py),
        SqlParam::Flag(value) => value.into_bound_py_any(py),
        SqlParam::Timestamp(value) => python_time(py, value),
        SqlParam::TextList(values) => Ok(PyList::new(py, values)?.into_any()),
    }
}

/// The parameters of a query as a tuple.
pub(crate) fn python_params<'py>(
    py: Python<'py>,
    params: &[SqlParam],
) -> PyResult<Bound<'py, PyTuple>> {
    let values: Vec<Bound<'py, PyAny>> = params
        .iter()
        .map(|param| python_param(py, param))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, values)
}

/// `uuid.uuid4().hex`.
pub(crate) fn new_namespace(py: Python<'_>) -> PyResult<String> {
    imported(py, "uuid", "uuid4")?
        .call0()?
        .getattr("hex")?
        .extract()
}
