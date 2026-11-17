//! Calls into the ClickHouse adapter's Recall store operations, through the native host.

use crate::recall::models::QueryStatistics;
use pyo3::prelude::{Bound, FromPyObject, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyBytes, PyDict, PyDictMethods, PyInt, PyList, PyString};
use sqlbuild_config::models::ConfigValue;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_recall::clickhouse::main::error_kind::error_kind;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::store::errors::{StoreError, StoreErrorKind};
use sqlbuild_recall::store::models::QueryResult;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::config_value_to_python::config_value_to_python;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_threads::constants::OUTPUT_SUPPRESSED_ATTRIBUTE;

/// One result cell as the adapter returns it: plain values only.
#[derive(FromPyObject)]
enum Cell {
    Integer(i64),
    Real(f64),
    Text(String),
    Bytes(Vec<u8>),
}

/// A query reply: column names and types, rows of cells, and rows read, bytes read and query ID.
type QueryReply = (
    (Vec<String>, Vec<Option<String>>),
    Vec<Vec<Option<Cell>>>,
    (Option<u64>, Option<u64>, Option<String>),
);

/// The adapter's handle for one open store connection; the connection itself stays in Python.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StoreHandle(u64);

/// A failure the adapter reported: whether it came from the ClickHouse client, and its message.
type Failure = Option<(bool, String)>;

/// A store failure from the adapter's report: client errors by server code, anything else as is.
fn reported(action: &str, (from_client, message): (bool, String)) -> StoreError {
    let kind: StoreErrorKind = if from_client {
        error_kind(&message)
    } else {
        StoreErrorKind::Other
    };
    StoreError {
        kind,
        message: format!(
            "cannot {action}: {}",
            message.lines().next().unwrap_or_default()
        ),
    }
}

fn raised(action: &str, error: &PyErr) -> StoreError {
    StoreError {
        kind: StoreErrorKind::Other,
        message: format!("cannot {action}: {error}"),
    }
}

/// Call the host entry `entry` with `arguments` and unpack its `(failure, value)` reply.
fn reply<'py>(
    py: Python<'py>,
    entry: AdapterHost,
    arguments: &[(&str, Bound<'py, PyAny>)],
    action: &str,
) -> Result<Bound<'py, PyAny>, StoreError> {
    let called: PyResult<(Failure, Bound<'py, PyAny>)> = host_entry(py, entry)
        .and_then(|callable| call_with(&callable, arguments))
        .and_then(|replied| replied.extract());
    match called {
        Ok((None, value)) => Ok(value),
        Ok((Some(failure), _)) => Err(reported(action, failure)),
        Err(error) => Err(raised(action, &error)),
    }
}

/// How a store connection is used: the delivery thread writes and is never captured; a reader skips the session settings under `readonly = 1`, so that user can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionRole {
    Writer,
    Reader,
}

/// Connect with the store's connection options for `role`.
pub(crate) fn connect(
    py: Python<'_>,
    (config, role): (&[(String, ConfigValue)], ConnectionRole),
    action: &str,
) -> Result<StoreHandle, StoreError> {
    let options = PyDict::new(py);
    for (key, value) in config {
        config_value_to_python(py, value.clone())
            .and_then(|value| options.set_item(key, value))
            .map_err(|error| raised(action, &error))?;
    }
    reply(
        py,
        AdapterHost::ConnectRecallStore,
        &[
            ("config", options.into_any()),
            (
                "quiet_thread_attribute",
                if role == ConnectionRole::Writer {
                    PyString::new(py, OUTPUT_SUPPRESSED_ATTRIBUTE).into_any()
                } else {
                    py.None().into_bound(py)
                },
            ),
            (
                "read_only",
                PyBool::new(py, role == ConnectionRole::Reader)
                    .to_owned()
                    .into_any(),
            ),
        ],
        action,
    )?
    .extract()
    .map(StoreHandle)
    .map_err(|error| raised(action, &error))
}

fn handle_value(py: Python<'_>, handle: StoreHandle) -> Bound<'_, PyAny> {
    PyInt::new(py, handle.0).into_any()
}

fn cell_value(cell: Option<Cell>) -> RecallValue {
    match cell {
        None => RecallValue::Null,
        Some(Cell::Integer(number)) => RecallValue::Integer(number),
        Some(Cell::Real(number)) => RecallValue::Real(number),
        Some(Cell::Text(text)) => RecallValue::Text(text),
        Some(Cell::Bytes(data)) => RecallValue::Blob(data),
    }
}

/// The columns, at most `row_cap` rows and the statistics of `sql`, run with those of `settings` the session does not already hold.
pub(crate) fn query(
    py: Python<'_>,
    handle: StoreHandle,
    (sql, settings, row_cap): (&str, &[(&str, i64)], Option<usize>),
    action: &str,
) -> Result<(QueryResult, QueryStatistics), StoreError> {
    let query_settings = PyDict::new(py);
    for (name, value) in settings {
        query_settings
            .set_item(*name, *value)
            .map_err(|error| raised(action, &error))?;
    }
    let value = reply(
        py,
        AdapterHost::RunRecallQuery,
        &[
            ("handle", handle_value(py, handle)),
            ("sql", PyString::new(py, sql).into_any()),
            ("settings", query_settings.into_any()),
            (
                "row_cap",
                row_cap.map_or_else(
                    || py.None().into_bound(py),
                    |cap| PyInt::new(py, cap).into_any(),
                ),
            ),
        ],
        action,
    )?;
    let ((columns, types), rows, (rows_read, bytes_read, query_id)): QueryReply =
        value.extract().map_err(|error| raised(action, &error))?;
    Ok((
        QueryResult {
            columns,
            types,
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().map(cell_value).collect())
                .collect(),
            truncated: false,
        },
        QueryStatistics {
            rows_read,
            bytes_read,
            query_id,
        },
    ))
}

/// Run one DDL statement.
pub(crate) fn command(
    py: Python<'_>,
    handle: StoreHandle,
    sql: &str,
    action: &str,
) -> Result<(), StoreError> {
    reply(
        py,
        AdapterHost::RunRecallCommand,
        &[
            ("handle", handle_value(py, handle)),
            ("sql", PyString::new(py, sql).into_any()),
        ],
        action,
    )
    .map(drop)
}

/// Insert one `RowBinary` block into `table` under `deduplication_token`.
pub(crate) fn insert(
    py: Python<'_>,
    handle: StoreHandle,
    (table, column_names, payload): (&str, &[String], &[u8]),
    deduplication_token: &str,
) -> Result<(), StoreError> {
    let action = "write Recall rows";
    let names = PyList::new(py, column_names).map_err(|error| raised(action, &error))?;
    reply(
        py,
        AdapterHost::InsertRecallRows,
        &[
            ("handle", handle_value(py, handle)),
            ("table", PyString::new(py, table).into_any()),
            ("column_names", names.into_any()),
            ("payload", PyBytes::new(py, payload).into_any()),
            (
                "deduplication_token",
                PyString::new(py, deduplication_token).into_any(),
            ),
        ],
        action,
    )
    .map(drop)
}

/// Close the connection; a failure to close is ignored.
pub(crate) fn close(py: Python<'_>, handle: StoreHandle) {
    drop(
        host_entry(py, AdapterHost::CloseRecallStore)
            .and_then(|callable| call_with(&callable, &[("handle", handle_value(py, handle))])),
    );
}
