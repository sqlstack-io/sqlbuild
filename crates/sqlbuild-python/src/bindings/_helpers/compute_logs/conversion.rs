//! Compute log metadata models and errors across the Python boundary.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyInt, PyString, PyStringMethods};
use sqlbuild_cli::compute_logs::errors::{ComputeLogError, ComputeLogErrorKind};
use sqlbuild_cli::compute_logs::models::{
    CaptureCompletion, CaptureDocument, CaptureInstant, InventoryEntry,
};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const EPOCH_TEXT: &str = "1970-01-01T00:00:00+00:00";
const UTC_OFFSET: &str = "+00:00";
const UTC_SUFFIX: &str = "Z";
const STREAM_VALUES: [&str; 3] = ["stdout", "stderr", "diagnostics"];

fn exception_class(kind: ComputeLogErrorKind) -> ExecutorHost {
    match kind {
        ComputeLogErrorKind::InvalidCaptureId => ExecutorHost::InvalidCaptureIdError,
        ComputeLogErrorKind::InvalidCursor => ExecutorHost::InvalidComputeLogCursorError,
        ComputeLogErrorKind::InvalidLimit => ExecutorHost::InvalidComputeLogLimitError,
        ComputeLogErrorKind::AlreadyExists => ExecutorHost::CaptureAlreadyExistsError,
        ComputeLogErrorKind::NotFound => ExecutorHost::CaptureNotFoundError,
        ComputeLogErrorKind::State => ExecutorHost::CaptureStateError,
        ComputeLogErrorKind::Path => ExecutorHost::ComputeLogPathError,
        ComputeLogErrorKind::Metadata => ExecutorHost::ComputeLogMetadataError,
    }
}

/// The Python exception for a storage rejection of `kind`.
pub(crate) fn rejection(py: Python<'_>, kind: ComputeLogErrorKind, message: &str) -> PyErr {
    host_error(py, exception_class(kind), message)
}

/// The Python exception a native storage failure raises.
pub(crate) fn storage_error(py: Python<'_>, error: ComputeLogError) -> PyErr {
    match error {
        ComputeLogError::Rejected(kind, message) => rejection(py, kind, &message),
        ComputeLogError::Io(error) => PyErr::from(error),
    }
}

/// Whether `value` is exactly an `int`, as `type(value) is int` checks.
pub(crate) fn is_exact_int(value: &Bound<'_, PyAny>) -> bool {
    value.get_type().is(value.py().get_type::<PyInt>())
}

/// `invocation_id` as text, rejecting non-strings as invalid capture ids.
pub(crate) fn capture_id(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = value.downcast::<PyString>() {
        return Ok(text.to_str()?.to_owned());
    }
    Err(rejection(
        value.py(),
        ComputeLogErrorKind::InvalidCaptureId,
        "invocation_id must contain only ASCII letters, digits, underscores, or hyphens",
    ))
}

/// The stream index of a `ComputeLogStream` member: stdout, stderr or diagnostics.
pub(crate) fn stream_index(stream: &Bound<'_, PyAny>) -> PyResult<usize> {
    let py = stream.py();
    if stream.is_instance(&host_entry(py, ExecutorHost::ComputeLogStream)?)? {
        let value: String = native_field(stream, "value")?.extract()?;
        if let Some(index) = STREAM_VALUES.iter().position(|name| *name == value) {
            return Ok(index);
        }
    }
    Err(rejection(
        py,
        ComputeLogErrorKind::State,
        "stream must be a ComputeLogStream",
    ))
}

fn instant(value: &Bound<'_, PyAny>) -> PyResult<CaptureInstant> {
    let py = value.py();
    let text: String = value.call_method0("isoformat")?.extract()?;
    let text = text.replace(UTC_OFFSET, UTC_SUFFIX);
    if native_field(value, "tzinfo")?.is_none() || value.call_method0("utcoffset")?.is_none() {
        return Ok(CaptureInstant {
            micros: 0,
            aware: false,
            text,
        });
    }
    let datetime = imported(py, "datetime", "datetime")?;
    let epoch = datetime.call_method1("fromisoformat", (EPOCH_TEXT,))?;
    let unit = call_with(
        &imported(py, "datetime", "timedelta")?,
        &[("microseconds", 1_i64.into_bound_py_any(py)?)],
    )?;
    let micros: i64 = value.sub(epoch)?.floor_div(unit)?.extract()?;
    Ok(CaptureInstant {
        micros,
        aware: true,
        text,
    })
}

fn count(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<i64> {
    let message = "exit_code and byte counts must be integers";
    if !is_exact_int(value) {
        return Err(rejection(py, ComputeLogErrorKind::Metadata, message));
    }
    value
        .extract()
        .map_err(|_| rejection(py, ComputeLogErrorKind::Metadata, message))
}

/// A Python `CaptureMetadata` or `FinalCaptureMetadata` as a native capture document.
pub(crate) fn document(metadata: &Bound<'_, PyAny>) -> PyResult<CaptureDocument> {
    let py = metadata.py();
    let invocation_id = capture_id(&native_field(metadata, "invocation_id")?)?;
    let version = native_field(metadata, "format_version")?;
    let format_version: i64 = if is_exact_int(&version) {
        version.extract().unwrap_or(0)
    } else {
        i64::from(version.eq(1)?)
    };
    let completion =
        if metadata.is_instance(&host_entry(py, ExecutorHost::FinalCaptureMetadata)?)? {
            Some(CaptureCompletion {
                completed_at: instant(&native_field(metadata, "completed_at")?)?,
                exit_code: count(py, &native_field(metadata, "exit_code")?)?,
                stdout_bytes: count(py, &native_field(metadata, "stdout_bytes")?)?,
                stderr_bytes: count(py, &native_field(metadata, "stderr_bytes")?)?,
                diagnostics_bytes: count(py, &native_field(metadata, "diagnostics_bytes")?)?,
            })
        } else {
            None
        };
    Ok(CaptureDocument {
        format_version,
        invocation_id,
        command: native_field(metadata, "command")?.extract()?,
        project_dir: native_field(metadata, "project_dir")?.extract()?,
        started_at: instant(&native_field(metadata, "started_at")?)?,
        capture_date: native_field(metadata, "capture_date")?.extract()?,
        target: native_field(metadata, "target")?.extract()?,
        run_id: native_field(metadata, "run_id")?.extract()?,
        completion,
    })
}

fn datetime_value<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "datetime", "datetime")?
        .call_method1("fromisoformat", (text.replace(UTC_SUFFIX, UTC_OFFSET),))
        .map_err(|error| {
            if error.is_instance_of::<PyValueError>(py) {
                rejection(
                    py,
                    ComputeLogErrorKind::Metadata,
                    &format!("invalid capture metadata: {}", error.value(py)),
                )
            } else {
                error
            }
        })
}

/// The Python `CaptureMetadata` or `FinalCaptureMetadata` of a native document.
pub(crate) fn metadata_model<'py>(
    py: Python<'py>,
    document: &CaptureDocument,
) -> PyResult<Bound<'py, PyAny>> {
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        (
            "format_version",
            document.format_version.into_bound_py_any(py)?,
        ),
        (
            "invocation_id",
            document.invocation_id.as_str().into_bound_py_any(py)?,
        ),
        ("command", document.command.as_str().into_bound_py_any(py)?),
        (
            "project_dir",
            document.project_dir.as_str().into_bound_py_any(py)?,
        ),
        ("started_at", datetime_value(py, &document.started_at.text)?),
        (
            "capture_date",
            document.capture_date.as_str().into_bound_py_any(py)?,
        ),
        ("target", document.target.as_deref().into_bound_py_any(py)?),
        ("run_id", document.run_id.as_deref().into_bound_py_any(py)?),
    ];
    let Some(completion) = &document.completion else {
        return call_with(&host_entry(py, ExecutorHost::CaptureMetadata)?, &fields);
    };
    fields.extend([
        (
            "completed_at",
            datetime_value(py, &completion.completed_at.text)?,
        ),
        ("exit_code", completion.exit_code.into_bound_py_any(py)?),
        (
            "stdout_bytes",
            completion.stdout_bytes.into_bound_py_any(py)?,
        ),
        (
            "stderr_bytes",
            completion.stderr_bytes.into_bound_py_any(py)?,
        ),
        (
            "diagnostics_bytes",
            completion.diagnostics_bytes.into_bound_py_any(py)?,
        ),
    ]);
    call_with(
        &host_entry(py, ExecutorHost::FinalCaptureMetadata)?,
        &fields,
    )
}

/// A Python `CaptureInventoryItem`.
pub(crate) fn inventory_item<'py>(
    py: Python<'py>,
    entry: &InventoryEntry,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(py, ExecutorHost::CaptureInventoryItem)?,
        &[
            (
                "invocation_id",
                entry
                    .document
                    .invocation_id
                    .as_str()
                    .into_bound_py_any(py)?,
            ),
            (
                "capture_date",
                entry.document.capture_date.as_str().into_bound_py_any(py)?,
            ),
            ("path", entry.path.to_string_lossy().into_bound_py_any(py)?),
            ("is_complete", entry.is_complete.into_bound_py_any(py)?),
            ("metadata", metadata_model(py, &entry.document)?),
        ],
    )
}
