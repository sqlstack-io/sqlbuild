//! The best-effort, append-only statement ledger that cost collection reads back.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyString, PyTuple, PyTypeMethods};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonInteger, JsonValue, StdlibJsonOptions};

use crate::cost_telemetry::_helpers::health::mark_ledger_failure;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cost_context::main::process_object::process_object;
use sqlbuild_python_front::cursor::constants::DATETIME_MODULE;

static LEDGER_LOCK: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The process-wide `threading.Lock` that orders ledger appends and snapshot reads.
fn ledger_lock(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    process_object(&LEDGER_LOCK, py, || {
        imported(py, "threading", "Lock")?.call0()
    })
}

fn mark_failure(run_id: &Bound<'_, PyAny>, error: &Bound<'_, PyAny>) -> PyResult<()> {
    mark_ledger_failure(run_id.extract()?, error)
}

fn optional_text(value: Option<String>) -> JsonValue {
    value.map_or(JsonValue::Null, JsonValue::String)
}

fn text(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    Ok(JsonValue::String(value.extract()?))
}

/// The ledger line of one statement: compact, key-sorted JSON.
fn ledger_line(arguments: &KeywordArguments<'_>, context: &Bound<'_, PyAny>) -> PyResult<String> {
    let query_id: Bound<'_, PyAny> = arguments.value("query_id")?;
    let query_id: Option<String> =
        if query_id.is_instance_of::<PyString>() && query_id.is_truthy()? {
            Some(query_id.extract()?)
        } else {
            None
        };
    let sql: String = arguments.value("sql")?.extract()?;
    let error: Bound<'_, PyAny> = arguments.value("error")?;
    let error_type: Option<String> = if error.is_none() {
        None
    } else {
        Some(error.get_type().name()?.extract()?)
    };
    let attempt: String = native_field(context, "attempt")?
        .extract::<i128>()?
        .to_string();
    let isoformat = |name: &str| -> PyResult<JsonValue> {
        text(&arguments.value(name)?.call_method0("isoformat")?)
    };
    let mut fields: Vec<(String, JsonValue)> = vec![
        (
            "run_id".to_owned(),
            text(&native_field(context, "run_id")?)?,
        ),
        (
            "statement_id".to_owned(),
            text(&arguments.value("statement_id")?)?,
        ),
        (
            "resource_type".to_owned(),
            text(&native_field(context, "resource_type")?)?,
        ),
        (
            "resource_name".to_owned(),
            text(&native_field(context, "resource_name")?)?,
        ),
        ("phase".to_owned(), text(&native_field(context, "phase")?)?),
        (
            "attempt".to_owned(),
            JsonValue::Integer(JsonInteger::parse(&attempt).ok_or_else(|| {
                PyValueError::new_err("statement ledger attempt must be an integer")
            })?),
        ),
        ("query_id".to_owned(), optional_text(query_id)),
        ("status".to_owned(), text(&arguments.value("status")?)?),
        ("started_at".to_owned(), isoformat("started_at")?),
        ("completed_at".to_owned(), isoformat("completed_at")?),
        (
            "sql_sha256".to_owned(),
            JsonValue::String(sha256_hex(sql.as_bytes())),
        ),
        ("error_type".to_owned(), optional_text(error_type)),
    ];
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    let options: StdlibJsonOptions = StdlibJsonOptions {
        sort_keys: true,
        ..StdlibJsonOptions::new(None).with_separators(",", ":")
    };
    dumps(&JsonValue::Object(fields), &JsonDialect::Stdlib(options))
        .map_err(|error| PyValueError::new_err(format!("{error:?}")))
}

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    file.write_all(format!("{line}\n").as_bytes())?;
    file.flush()
}

/// Append statement identity without allowing telemetry I/O to affect execution.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn record_statement(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<()> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "record_statement",
        options,
        (
            &[
                "context",
                "statement_id",
                "sql",
                "query_id",
                "status",
                "started_at",
                "completed_at",
            ],
            &["error"],
        ),
    )?;
    let context: Bound<'_, PyAny> = arguments.value("context")?;
    let path: Bound<'_, PyAny> = native_field(&context, "ledger_path")?;
    if path.is_none() {
        return Ok(());
    }
    let appended: PyResult<()> = (|| {
        let line: String = ledger_line(&arguments, &context)?;
        let path: PathBuf = path.extract()?;
        within(ledger_lock(py)?, |_| {
            append_line(&path, &line).map_err(PyErr::from)
        })
    })();
    match appended {
        Ok(()) => Ok(()),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyException>(py) => {
            mark_failure(&native_field(&context, "run_id")?, error.value(py).as_any())
        }
        Err(error) => Err(error),
    }
}

/// One ledger line as a `StatementLedgerEntry`, raising the error Python's parsing raises.
fn ledger_entry<'py>(line: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = line.py();
    let payload: Bound<'py, PyAny> = imported(py, "json", "loads")?.call1((line,))?;
    let query_id: Bound<'py, PyAny> = mapping_get(&payload, "query_id")?;
    let field = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        Ok(payload.get_item(name)?.str()?.into_any())
    };
    let datetime: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let parsed = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        datetime.call_method1("fromisoformat", (field(name)?,))
    };
    let entry_fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("statement_id", field("statement_id")?),
        ("run_id", field("run_id")?),
        ("resource_type", field("resource_type")?),
        ("resource_name", field("resource_name")?),
        ("phase", field("phase")?),
        (
            "attempt",
            py.get_type::<pyo3::types::PyInt>()
                .call1((payload.get_item("attempt")?,))?,
        ),
        (
            "query_id",
            if query_id.is_instance_of::<PyString>() && query_id.is_truthy()? {
                query_id
            } else {
                py.None().into_bound(py)
            },
        ),
        ("status", field("status")?),
        ("started_at", parsed("started_at")?),
        ("completed_at", parsed("completed_at")?),
    ];
    new_record_with(py, RecordId::StatementLedgerEntry, &entry_fields)
}

fn entry_error(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyKeyError>(py)
        || error.is_instance_of::<PyTypeError>(py)
        || error.is_instance_of::<PyValueError>(py)
}

/// Valid ledger entries of a stable snapshot, marking invalid lines and read failures.
pub(crate) fn read_ledger<'py>(
    path: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = path.py();
    if !path.call_method0("is_file")?.is_truthy()? {
        return Ok(Vec::new());
    }
    let native_path: PathBuf = path.extract()?;
    let read: std::io::Result<Vec<u8>> = within(ledger_lock(py)?, |_| Ok(fs::read(&native_path)))?;
    let bytes: Vec<u8> = match read {
        Ok(bytes) => bytes,
        Err(error) => {
            mark_failure(run_id, PyErr::from(error).value(py).as_any())?;
            return Ok(Vec::new());
        }
    };
    let content: Bound<'py, PyAny> = PyBytes::new(py, &bytes).call_method1("decode", ("utf-8",))?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for line in content.call_method0("splitlines")?.try_iter()? {
        match ledger_entry(&line?) {
            Ok(entry) => entries.push(entry),
            Err(error) if entry_error(py, &error) => {
                let message: String = format!(
                    "invalid statement ledger entry ({})",
                    error.get_type(py).name()?
                );
                let artifact_error: Bound<'py, PyAny> =
                    native_attr(py, NativeAttr::CostArtifactError)?.call1((message,))?;
                mark_failure(run_id, &artifact_error)?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(entries)
}

/// Read a stable snapshot of valid statement ledger entries.
#[pyfunction]
#[pyo3(signature = (*, path, run_id))]
pub(crate) fn read_statement_ledger<'py>(
    path: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(path.py(), read_ledger(path, run_id)?)
}
