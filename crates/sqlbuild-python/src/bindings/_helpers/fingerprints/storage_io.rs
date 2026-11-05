//! Fingerprint table reads and append-only writes through the adapter's execute callable.

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::fingerprint_storage::main::create_table_sql::create_table_sql;
use sqlbuild_analysis::fingerprint_storage::main::read_latest_sql::read_latest_sql;
use sqlbuild_analysis::fingerprint_storage::main::relevant_read_sql::relevant_read_sql;
use sqlbuild_analysis::fingerprint_storage::models::RelevantRead;

use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::fingerprints::latest_rows::{
    fingerprint_input_error, fingerprint_set_from_rows,
};
use crate::bindings::_helpers::fingerprints::storage_sql::{
    fingerprint_insert_batches, fingerprint_insert_sql,
};
use crate::bindings::_helpers::records::construct::new_record;

use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::state_column_evolution::ensure_event_id_column;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
const TIME_MODULE: &str = "time";
const LOGGING_MODULE: &str = "logging";
const LOGGER_NAME: &str = "sqlbuild.fingerprints";
const WRITE_ATTEMPTS: usize = 5;
const WRITE_RETRY_BASE_SECONDS: f64 = 0.05;
/// `(rows per insert, bytes per insert)`.
const WRITE_BATCH_LIMITS: (i64, i64) = (50, 512_000);
const MISSING_SCHEMA: &str = "fingerprint table requires a target schema";
const SINGLE_RETRY_MESSAGE: &str = "fingerprint write attempt %s/%s failed for %s '%s' \
    (likely concurrent-writer conflict); retrying: %s: %s";
const BATCH_RETRY_MESSAGE: &str = "fingerprint batch write attempt %s/%s failed for %s row(s), \
    starting with %s '%s' (likely concurrent-writer conflict); retrying: %s: %s";

/// Where the fingerprint table lives and how the adapter names it.
struct Location<'a, 'py> {
    database: Option<&'a str>,
    schema: &'a str,
    render_qualified_name: &'a Bound<'py, PyAny>,
}

/// How to run SQL against the warehouse and render the fingerprint table DDL.
struct Writer<'a, 'py> {
    connection: &'a Bound<'py, PyAny>,
    execute: &'a Bound<'py, PyAny>,
    render_framework_type: &'a Bound<'py, PyAny>,
    render_create_table_sql: Option<&'a Bound<'py, PyAny>>,
    render_create_index_sqls: Option<&'a Bound<'py, PyAny>>,
}

fn location_keywords<'py>(
    py: Python<'py>,
    database: Option<&str>,
    schema: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("database", database)?;
    keywords.set_item("schema", schema)?;
    Ok(keywords)
}

fn qualified_table_name(location: &Location<'_, '_>) -> PyResult<String> {
    let py: Python<'_> = location.render_qualified_name.py();
    let keywords: Bound<'_, PyDict> = location_keywords(py, location.database, location.schema)?;
    keywords.set_item("name", FINGERPRINT_TABLE_NAME)?;
    let rendered: Option<String> = location
        .render_qualified_name
        .call((), Some(&keywords))?
        .extract()?;
    rendered.ok_or_else(|| fingerprint_input_error(py, MISSING_SCHEMA))
}

fn create_sql(
    location: &Location<'_, '_>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py: Python<'_> = render_framework_type.py();
    let framework: Bound<'_, PyAny> = host::entry(py, AdapterHost::FrameworkType)?;
    let string_type: String = render_framework_type
        .call1((native_field(&framework, "STRING")?,))?
        .extract()?;
    let timestamp_type: String = render_framework_type
        .call1((native_field(&framework, "TIMESTAMP")?,))?
        .extract()?;
    Ok(create_table_sql(
        &qualified_table_name(location)?,
        &string_type,
        &timestamp_type,
    ))
}

/// Create the table/indexes without an extra catalog read for a current or new table.
fn setup<'py>(
    location: &Location<'_, 'py>,
    writer: &Writer<'_, 'py>,
    describe: &dyn Fn(&PyErr) -> PyResult<RetryWarning<'py>>,
) -> PyResult<()> {
    let py: Python<'_> = writer.execute.py();
    let mut sqls: Vec<String> = vec![match writer.render_create_table_sql {
        Some(render) => render
            .call(
                (),
                Some(&location_keywords(py, location.database, location.schema)?),
            )?
            .extract()?,
        None => create_sql(location, writer.render_framework_type)?,
    }];
    if let Some(render) = writer.render_create_index_sqls {
        let indexes: Bound<'_, PyAny> = render.call(
            (),
            Some(&location_keywords(py, location.database, location.schema)?),
        )?;
        for sql in indexes.try_iter()? {
            sqls.push(sql?.extract()?);
        }
    }
    retry_write(
        py,
        || {
            for sql in &sqls {
                execute_sql(writer, sql)?;
            }
            Ok(())
        },
        describe,
    )
}

fn execute_sql<'py>(writer: &Writer<'_, 'py>, sql: &str) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(writer.execute.py());
    keywords.set_item("connection", writer.connection)?;
    keywords.set_item("sql", sql)?;
    writer.execute.call((), Some(&keywords))
}

/// A failed insert triggers catalog-confirmed additive evolution inside the write retry budget.
fn execute_insert<'py>(
    location: &Location<'_, 'py>,
    writer: &Writer<'_, 'py>,
    request: &Request<'_, 'py>,
    sql: &str,
) -> PyResult<()> {
    let error = match execute_sql(writer, sql) {
        Ok(_) => return Ok(()),
        Err(error) => error,
    };
    let py = writer.execute.py();
    if !error.is_instance_of::<PyException>(py) || present(request, "read_columns")?.is_none() {
        return Err(error);
    }
    ensure_event_id_column(
        (writer.connection, writer.execute),
        (
            &location.database.into_pyobject(py)?.into_any(),
            &location.schema.into_pyobject(py)?.into_any(),
        ),
        (FINGERPRINT_TABLE_NAME, &qualified_table_name(location)?),
        (
            &request.get("read_columns")?,
            &request.get("render_add_columns")?,
            writer.render_framework_type,
        ),
    )?;
    execute_sql(writer, sql).map(|_| ())
}

/// Log one failed attempt with `details` after the attempt counts, then back off.
fn retry_pause(
    py: Python<'_>,
    (message, attempt): (&str, usize),
    details: Vec<Bound<'_, PyAny>>,
) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = py
        .import(LOGGING_MODULE)?
        .call_method1("getLogger", (LOGGER_NAME,))?;
    let mut arguments: Vec<Bound<'_, PyAny>> = vec![
        message.into_pyobject(py)?.into_any(),
        (attempt + 1).into_pyobject(py)?.into_any(),
        WRITE_ATTEMPTS.into_pyobject(py)?.into_any(),
    ];
    arguments.extend(details);
    logger.call_method1("warning", PyTuple::new(py, arguments)?)?;
    let seconds: f64 =
        WRITE_RETRY_BASE_SECONDS * f64::from(u32::try_from(attempt + 1).unwrap_or(u32::MAX));
    py.import(TIME_MODULE)?.call_method1("sleep", (seconds,))?;
    Ok(())
}

/// The details a retry warning names: the error type and the error itself.
fn error_details<'py>(py: Python<'py>, error: &PyErr) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    Ok(vec![value.get_type().getattr("__name__")?, value])
}

/// Retry a complete warehouse operation after transient exceptions within the write budget.
fn retry_write<'py>(
    py: Python<'py>,
    write: impl Fn() -> PyResult<()>,
    describe: &dyn Fn(&PyErr) -> PyResult<RetryWarning<'py>>,
) -> PyResult<()> {
    for attempt in 0..WRITE_ATTEMPTS {
        let Err(error) = write() else {
            return Ok(());
        };
        if attempt + 1 == WRITE_ATTEMPTS || !error.is_instance_of::<PyException>(py) {
            return Err(error);
        }
        let (message, details) = describe(&error)?;
        retry_pause(py, (message, attempt), details)?;
    }
    Ok(())
}

/// A retry warning's message and the details it names after the attempt counts.
type RetryWarning<'py> = (&'static str, Vec<Bound<'py, PyAny>>);

/// The owned keyword arguments a fingerprint write reads.
struct WriteParts<'py> {
    connection: Bound<'py, PyAny>,
    execute: Bound<'py, PyAny>,
    database: Option<String>,
    schema: String,
    render_qualified_name: Bound<'py, PyAny>,
    render_framework_type: Bound<'py, PyAny>,
    render_create_table_sql: Option<Bound<'py, PyAny>>,
    render_create_index_sqls: Option<Bound<'py, PyAny>>,
}

impl<'py> WriteParts<'py> {
    fn read(request: &Request<'_, 'py>) -> PyResult<Self> {
        Ok(Self {
            connection: request.get("connection")?,
            execute: request.get("execute")?,
            database: request.get("database")?.extract()?,
            schema: request.get("schema")?.extract()?,
            render_qualified_name: request.get("render_qualified_name")?,
            render_framework_type: request.get("render_framework_type")?,
            render_create_table_sql: present(request, "render_create_table_sql")?,
            render_create_index_sqls: present(request, "render_create_index_sqls")?,
        })
    }

    fn location(&self) -> Location<'_, 'py> {
        Location {
            database: self.database.as_deref(),
            schema: &self.schema,
            render_qualified_name: &self.render_qualified_name,
        }
    }

    fn writer(&self) -> Writer<'_, 'py> {
        Writer {
            connection: &self.connection,
            execute: &self.execute,
            render_framework_type: &self.render_framework_type,
            render_create_table_sql: self.render_create_table_sql.as_ref(),
            render_create_index_sqls: self.render_create_index_sqls.as_ref(),
        }
    }
}

/// An optional argument the caller passed as something other than `None`.
fn present<'py>(request: &Request<'_, 'py>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(request.optional(name)?.filter(|value| !value.is_none()))
}

/// Fingerprint-table DDL for a target schema.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
fn build_create_table_sql(
    database: Option<&str>,
    schema: &str,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let location = Location {
        database,
        schema,
        render_qualified_name,
    };
    create_sql(&location, render_framework_type)
}

/// The windowed latest-row read of a target schema's fingerprint table.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name))]
fn build_read_latest_sql(
    database: Option<&str>,
    schema: &str,
    render_qualified_name: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let location = Location {
        database,
        schema,
        render_qualified_name,
    };
    Ok(read_latest_sql(&qualified_table_name(&location)?))
}

fn empty_set<'py>(py: Python<'py>, schema: &str) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("schema", schema)?;
    keywords.set_item("fingerprints", PyDict::new(py))?;
    new_record(py, RecordId::FingerprintSet, (), Some(&keywords))
}

/// `FingerprintInputError` for an unreadable table, caused by the warehouse error.
fn unreadable_table(py: Python<'_>, qualified_name: &str, error: PyErr) -> PyErr {
    let failure: PyErr = fingerprint_input_error(
        py,
        &format!(
            "Unable to read fingerprints from {qualified_name}. This can happen after \
             upgrading from an older sqlbuild version; delete or rebuild the SQLBuild \
             fingerprint table to regenerate fingerprints."
        ),
    );
    failure.set_cause(py, Some(error));
    failure
}

/// The latest fingerprint per node identity, read with the adapter-rendered latest-row SQL.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn read_latest_fingerprints<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("read_latest_fingerprints", request)?;
    let py: Python<'py> = request.py();
    let database: Option<String> = request.get("database")?.extract()?;
    let schema: String = request.get("schema")?.extract()?;
    let render_qualified_name: Bound<'py, PyAny> = request.get("render_qualified_name")?;
    let location = Location {
        database: database.as_deref(),
        schema: &schema,
        render_qualified_name: &render_qualified_name,
    };
    let qualified_name: String = qualified_table_name(&location)?;
    if !request.get("table_exists")?.extract::<bool>()? {
        let require_table: bool = match present(&request, "require_table")? {
            Some(value) => value.is_truthy()?,
            None => false,
        };
        if require_table {
            return Err(fingerprint_input_error(
                py,
                &format!("Unable to read fingerprints from {qualified_name}"),
            ));
        }
        return empty_set(py, &schema);
    }
    let read_sql: String = request
        .get("render_read_latest_sql")?
        .call(
            (),
            Some(&location_keywords(py, database.as_deref(), &schema)?),
        )?
        .extract()?;
    let node_names: Option<Vec<String>> = match present(&request, "node_names")? {
        Some(names) => Some(names.extract()?),
        None => None,
    };
    let filtered_node_types: Vec<String> = match present(&request, "filtered_node_types")? {
        Some(types) => types.extract()?,
        None => Vec::new(),
    };
    let RelevantRead::Sql(relevant_sql) =
        relevant_read_sql(&read_sql, node_names.as_deref(), &filtered_node_types)
    else {
        return empty_set(py, &schema);
    };
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("connection", request.get("connection")?)?;
    keywords.set_item("sql", relevant_sql)?;
    let result: Bound<'py, PyAny> = match request.get("execute")?.call((), Some(&keywords)) {
        Ok(result) => result,
        Err(error) if error.is_instance_of::<PyException>(py) => {
            return Err(unreadable_table(py, &qualified_name, error));
        }
        Err(error) => return Err(error),
    };
    let rows: Bound<'py, PyAny> = result.call_method0("fetchall")?;
    fingerprint_set_from_rows(py, &schema, &qualified_name, &rows)
}

/// Append one fingerprint row, retrying transient concurrent-writer conflicts. With `ensure_table` false the table is assumed to exist and is set up only after the insert fails; returns whether the write set up the table.
#[pyfunction]
#[pyo3(signature = (**request))]
fn write_fingerprint<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<bool> {
    let request = Request::new("write_fingerprint", request)?;
    let py: Python<'py> = request.py();
    let fingerprint: Bound<'py, PyAny> = request.get("fingerprint")?;
    let ensure_table: bool = match present(&request, "ensure_table")? {
        Some(value) => value.is_truthy()?,
        None => true,
    };
    let parts: WriteParts<'py> = WriteParts::read(&request)?;
    let (location, writer) = (parts.location(), parts.writer());
    let insert: String =
        fingerprint_insert_sql(py, &qualified_table_name(&location)?, &fingerprint)?;
    if !ensure_table {
        match execute_sql(&writer, &insert) {
            Ok(_) => return Ok(false),
            Err(error) if error.is_instance_of::<PyException>(py) => {}
            Err(error) => return Err(error),
        }
    }
    let describe = |error: &PyErr| {
        let mut details: Vec<Bound<'py, PyAny>> = vec![
            native_field(&fingerprint, "node_type")?,
            native_field(&fingerprint, "node_name")?,
        ];
        details.extend(error_details(py, error)?);
        Ok((SINGLE_RETRY_MESSAGE, details))
    };
    setup(&location, &writer, &describe)?;
    retry_write(
        py,
        || execute_insert(&location, &writer, &request, &insert),
        &describe,
    )?;
    Ok(true)
}

/// The batch warning details: row count, the first row's identity, then the error.
fn batch_warning<'py>(batch: &[Bound<'py, PyAny>], error: &PyErr) -> PyResult<RetryWarning<'py>> {
    let Some(first) = batch.first() else {
        return Err(PyValueError::new_err("empty fingerprint batch"));
    };
    let py: Python<'py> = first.py();
    let mut details: Vec<Bound<'py, PyAny>> = vec![
        batch.len().into_pyobject(py)?.into_any(),
        native_field(first, "node_type")?,
        native_field(first, "node_name")?,
    ];
    details.extend(error_details(py, error)?);
    Ok((BATCH_RETRY_MESSAGE, details))
}

/// Append fingerprints with one table setup and bounded multi-row inserts.
#[pyfunction]
#[pyo3(signature = (**request))]
fn write_fingerprints<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<()> {
    let request = Request::new("write_fingerprints", request)?;
    let py: Python<'py> = request.py();
    let fingerprints: Vec<Bound<'py, PyAny>> = request
        .get("fingerprints")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    if fingerprints.is_empty() {
        return Ok(());
    }
    let on_progress: Option<Bound<'py, PyAny>> = present(&request, "on_progress")?;
    let parts: WriteParts<'py> = WriteParts::read(&request)?;
    let (location, writer) = (parts.location(), parts.writer());
    setup(&location, &writer, &|error| {
        batch_warning(&fingerprints, error)
    })?;
    let batches = fingerprint_insert_batches(
        py,
        &qualified_table_name(&location)?,
        fingerprints.clone(),
        WRITE_BATCH_LIMITS,
    )?;
    let total: usize = fingerprints.len();
    let mut completed: usize = 0;
    for (batch, insert_sql) in batches {
        let members: Vec<Bound<'py, PyAny>> = batch.iter().collect();
        retry_write(
            py,
            || execute_insert(&location, &writer, &request, &insert_sql),
            &|error| batch_warning(&members, error),
        )?;
        completed += members.len();
        if let Some(progress) = &on_progress {
            let keywords: Bound<'py, PyDict> = PyDict::new(py);
            keywords.set_item("completed", completed)?;
            keywords.set_item("total", total)?;
            progress.call((), Some(&keywords))?;
        }
    }
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_create_table_sql, module)?)?;
    module.add_function(wrap_pyfunction!(build_read_latest_sql, module)?)?;
    module.add_function(wrap_pyfunction!(read_latest_fingerprints, module)?)?;
    module.add_function(wrap_pyfunction!(write_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(write_fingerprints, module)?)?;
    Ok(())
}
