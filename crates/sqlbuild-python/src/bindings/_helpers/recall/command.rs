//! `sqb recall query | schema | output`.

use std::path::PathBuf;
use std::time::Instant;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyStringMethods};
use sqlbuild_core::command_output::main::short_invocation_id::short_invocation_id;
use sqlbuild_recall::records::models::OutputStream;
use sqlbuild_recall::schema::constants::VIEW_NAMES;
use sqlbuild_recall::schema::main::ddl_statements::ddl_statements;
use sqlbuild_recall::schema::main::render_script::render_script;
use sqlbuild_recall::schema::main::upgrade_statements::upgrade_statements;
use sqlbuild_recall::schema::models::{DdlOptions, Dialect};
use sqlbuild_recall::settings::main::resolve_recall_settings::resolve_recall_settings;
use sqlbuild_recall::settings::models::{
    RecallSettings, SettingsRequest, StoreLocation, StorePurpose,
};
use sqlbuild_recall::store::errors::{StoreError, StoreErrorKind};
use sqlbuild_recall::store::main::describe_store::describe_store;
use sqlbuild_recall::store::main::observe_sqlite_schema::observe_sqlite_schema;
use sqlbuild_recall::store::main::open_sqlite_store::open_sqlite_store;
use sqlbuild_recall::store::main::query_store::query_store;
use sqlbuild_recall::store::main::read_output_transcript::read_output_transcript;
use sqlbuild_recall::store::main::resolve_invocation::resolve_invocation;
use sqlbuild_recall::store::models::{OpenMode, SqliteStore};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::query_command::query::run_query_command;
use crate::bindings::_helpers::recall::clickhouse_store::{ClickHouseLocation, ClickHouseReader};
use crate::bindings::_helpers::recall::console::{write_stderr, write_stdout_bytes};
use crate::bindings::_helpers::recall::constants::{
    DEFAULT_MAX_CELL_BYTES, DEFAULT_QUERY_LIMIT, JSON_FORMAT, LAST_INVOCATION,
    RECALL_AMBIGUOUS_CODE, RECALL_DISABLED_CODE, RECALL_NOT_FOUND_CODE, RECALL_OPTIONS,
    RECALL_QUERY_CODE, RECALL_SETTINGS_CODE, RECALL_STORE_CODE, WAREHOUSE_BACKEND,
};
use crate::bindings::_helpers::recall::rendering::{QueryContext, render_result, render_schema};
use crate::bindings::_helpers::records::construct::new_error;
use crate::bindings::_helpers::records::ids::ErrorId;

fn user_error(py: Python<'_>, message: &str, code: &str) -> PyErr {
    new_error(py, ErrorId::CliUser, message.to_owned(), code)
}

fn store_failure(py: Python<'_>, error: &StoreError) -> PyErr {
    let code = match error.kind {
        StoreErrorKind::NotFound => RECALL_NOT_FOUND_CODE,
        StoreErrorKind::Ambiguous => RECALL_AMBIGUOUS_CODE,
        StoreErrorKind::Query => RECALL_QUERY_CODE,
        _ => RECALL_STORE_CODE,
    };
    user_error(py, &error.message, code)
}

/// The options of one `sqb recall` invocation, as the dispatch route passes them.
struct RecallRequest {
    command: Option<String>,
    project_dir: Option<PathBuf>,
    selected_target: Option<String>,
    sql: Option<String>,
    backend: Option<String>,
    /// `long`, `json`, or `None` for the default layout.
    format: Option<String>,
    limit: Option<i64>,
    no_limit: bool,
    max_cell_bytes: Option<i64>,
    relation: Option<String>,
    invocation: Option<String>,
    stream: Option<String>,
    tail: Option<i64>,
    ddl: bool,
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.to_string_lossy().into_owned()))
}

fn recall_request(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<RecallRequest> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "run_recall_command", options, (&RECALL_OPTIONS, &[]))?;
    let text = |name: &str| optional_text(&arguments.value(name)?);
    let number = |name: &str| -> PyResult<Option<i64>> { arguments.value(name)?.extract() };
    Ok(RecallRequest {
        command: text("recall_command")?,
        project_dir: text("project_dir")?.map(PathBuf::from),
        selected_target: text("selected_target")?,
        sql: text("sql")?,
        backend: text("backend")?,
        format: text("output_format")?,
        limit: number("limit")?,
        no_limit: arguments.value("no_limit")?.is_truthy()?,
        max_cell_bytes: number("max_cell_bytes")?,
        relation: text("relation")?,
        invocation: text("invocation")?,
        stream: text("stream")?,
        tail: number("tail")?,
        ddl: arguments.value("ddl")?.is_truthy()?,
    })
}

/// The project directory, target and settings, or the coded failure.
fn settings(py: Python<'_>, recall: &RecallRequest) -> PyResult<(PathBuf, RecallSettings)> {
    let raw: PathBuf = match &recall.project_dir {
        Some(directory) => directory.clone(),
        None => std::env::current_dir()?,
    };
    let project_dir = std::fs::canonicalize(&raw).unwrap_or(raw);
    let request = SettingsRequest {
        project_dir: project_dir.clone(),
        selected_target: recall.selected_target.clone(),
        purpose: StorePurpose::Read,
    };
    let settings = resolve_recall_settings(&request)
        .map_err(|error| user_error(py, &error.message, RECALL_SETTINGS_CODE))?;
    if !settings.enabled {
        let target = settings
            .target
            .as_deref()
            .map_or_else(String::new, |target| format!(" for target `{target}`"));
        return Err(user_error(
            py,
            &format!("Recall is disabled{target}; set enabled = true in its recall settings"),
            RECALL_DISABLED_CODE,
        ));
    }
    Ok((project_dir, settings))
}

/// An open store for reading: the native SQLite file or a ClickHouse database.
enum ReadStore {
    Sqlite { store: SqliteStore, path: String },
    ClickHouse(ClickHouseReader),
}

impl ReadStore {
    fn dialect(&self) -> Dialect {
        match self {
            Self::Sqlite { .. } => Dialect::Sqlite,
            Self::ClickHouse(_) => Dialect::ClickHouse,
        }
    }

    fn adapter(&self) -> &'static str {
        match self.dialect() {
            Dialect::Sqlite => "sqlite",
            Dialect::ClickHouse => "clickhouse",
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Sqlite { path, .. } => path.clone(),
            Self::ClickHouse(reader) => reader.database().to_owned(),
        }
    }

    fn resolve_invocation(&self, py: Python<'_>, selector: &str) -> Result<String, StoreError> {
        match self {
            Self::Sqlite { store, .. } => resolve_invocation(store, selector),
            Self::ClickHouse(reader) => reader.resolve_invocation(py, selector),
        }
    }

    fn close(self) {
        if let Self::ClickHouse(reader) = self {
            reader.close();
        }
    }
}

/// The read store; `checked` first confirms a ClickHouse store exists, which a query skips and confirms only if it fails.
fn open_store(py: Python<'_>, settings: &RecallSettings, checked: bool) -> PyResult<ReadStore> {
    match &settings.store {
        StoreLocation::Sqlite { path } => {
            let store = open_sqlite_store(path, OpenMode::ReadOnly).map_err(|error| {
                if error.kind == StoreErrorKind::NotFound {
                    user_error(
                        py,
                        &format!(
                            "no Recall history is recorded yet at {}; run a command first",
                            path.display()
                        ),
                        RECALL_STORE_CODE,
                    )
                } else {
                    store_failure(py, &error)
                }
            })?;
            Ok(ReadStore::Sqlite {
                store,
                path: path.display().to_string(),
            })
        }
        StoreLocation::ClickHouse {
            config, database, ..
        } => ClickHouseReader::open(
            py,
            &ClickHouseLocation::new(config, database, settings.retention_days),
        )
        .and_then(|reader| {
            if checked && let Err(error) = reader.require_store(py) {
                reader.close();
                return Err(error);
            }
            Ok(reader)
        })
        .map(ReadStore::ClickHouse)
        .map_err(|error| {
            let error = if error.kind == StoreErrorKind::NotFound {
                error
            } else {
                StoreError {
                    kind: StoreErrorKind::Other,
                    ..error
                }
            };
            user_error(py, &error.message, RECALL_STORE_CODE)
        }),
        StoreLocation::Unsupported {
            connection,
            adapter,
        } => Err(user_error(
            py,
            &format!(
                "connection `{connection}` uses adapter `{adapter}`, which this version cannot \
                 read Recall history from"
            ),
            RECALL_STORE_CODE,
        )),
    }
}

/// The "no history yet" failure when a query failed because a writer is still creating the store's views, so it reads like the store not existing yet rather than an invalid query.
fn not_created(store: &SqliteStore, path: &str) -> Option<StoreError> {
    let complete: bool = match observe_sqlite_schema(store) {
        Ok(observed) => VIEW_NAMES.iter().all(|view| observed.views.contains(*view)),
        Err(_unreadable) => return None,
    };
    (!complete).then(|| StoreError {
        kind: StoreErrorKind::Other,
        message: format!(
            "no Recall history is recorded yet at {path}, or its tables are still being created; \
             run a command first"
        ),
    })
}

/// `sqb recall query --backend warehouse` and `schema --backend warehouse` run through `sqb query`.
fn warehouse_query(
    py: Python<'_>,
    recall: &RecallRequest,
    project_dir: &std::path::Path,
    (sql, limit): (&str, Option<i64>),
) -> PyResult<i32> {
    let options = PyDict::new(py);
    options.set_item("project_dir", project_dir)?;
    options.set_item("sql", sql)?;
    options.set_item("selected_target", recall.selected_target.as_deref())?;
    options.set_item("output_format", &recall.format)?;
    options.set_item("limit", limit)?;
    run_query_command(py, Some(options))
}

fn limit(recall: &RecallRequest) -> Option<i64> {
    if recall.no_limit {
        return None;
    }
    recall.limit.or(Some(DEFAULT_QUERY_LIMIT))
}

fn run_query(py: Python<'_>, recall: &RecallRequest) -> PyResult<i32> {
    let (project_dir, settings) = settings(py, recall)?;
    let sql: String = recall.sql.clone().unwrap_or_default();
    if sql.trim().is_empty() {
        return Err(user_error(
            py,
            "recall query SQL must not be empty",
            RECALL_QUERY_CODE,
        ));
    }
    let limit = limit(recall);
    if limit.is_some_and(|limit| limit < 0) {
        return Err(user_error(
            py,
            "--limit must not be negative",
            RECALL_QUERY_CODE,
        ));
    }
    if recall.backend.as_deref() == Some(WAREHOUSE_BACKEND) {
        return warehouse_query(py, recall, &project_dir, (&sql, limit));
    }
    let store = open_store(py, &settings, false)?;
    let started = Instant::now();
    let limit = limit.map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
    let queried = match &store {
        ReadStore::Sqlite { store, path } => query_store(store, &sql, limit)
            .map(|result| (result, None))
            .map_err(|failure| not_created(store, path).unwrap_or(failure)),
        ReadStore::ClickHouse(reader) => reader
            .query(py, &sql, limit)
            .map(|(result, statistics)| (result, Some(statistics))),
    };
    let (adapter, label) = (store.adapter(), store.label());
    store.close();
    let (result, statistics) = queried.map_err(|error| store_failure(py, &error))?;
    let format: Option<&str> = recall.format.as_deref();
    let max_bytes: Option<usize> = match recall.max_cell_bytes {
        Some(bytes) if bytes <= 0 => None,
        Some(bytes) => Some(usize::try_from(bytes).unwrap_or(usize::MAX)),
        None if format == Some(JSON_FORMAT) => None,
        None => Some(DEFAULT_MAX_CELL_BYTES),
    };
    let rendered = render_result(
        py,
        &result,
        (format, max_bytes),
        &QueryContext {
            target: settings.target.as_deref(),
            store_path: &label,
            adapter,
            limit,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            statistics: statistics.as_ref(),
        },
    )?;
    write_stdout_bytes(py, rendered.as_bytes())?;
    Ok(0)
}

/// Print what creates the read store, or brings it up to this build's schema, without changing it.
fn run_schema_ddl(py: Python<'_>, settings: &RecallSettings) -> PyResult<i32> {
    let (statements, place): (Result<Vec<String>, StoreError>, String) = match &settings.store {
        StoreLocation::Sqlite { path } => {
            let options = DdlOptions {
                database: None,
                retention_days: settings.retention_days,
                cluster: None,
                producer_version: env!("CARGO_PKG_VERSION").to_owned(),
            };
            let statements = if path.exists() {
                open_sqlite_store(path, OpenMode::ReadOnly)
                    .and_then(|store| observe_sqlite_schema(&store))
                    .map(|observed| upgrade_statements(Dialect::Sqlite, &options, &observed))
            } else {
                Ok(ddl_statements(Dialect::Sqlite, &options))
            };
            (statements, format!("at {}", path.display()))
        }
        StoreLocation::ClickHouse {
            config, database, ..
        } => (
            ClickHouseReader::upgrade_statements(
                py,
                &ClickHouseLocation::new(config, database, settings.retention_days),
            ),
            format!("in ClickHouse database `{database}`"),
        ),
        StoreLocation::Unsupported { .. } => {
            return open_store(py, settings, true).map(|store| {
                store.close();
                0
            });
        }
    };
    let statements =
        statements.map_err(|error| user_error(py, &error.message, RECALL_STORE_CODE))?;
    if statements.is_empty() {
        write_stderr(
            py,
            &format!(
                "The Recall store {place} is up to date with this SQLBuild version; there is \
                 nothing to run.\n"
            ),
        )?;
    } else {
        write_stdout_bytes(py, render_script(&statements).as_bytes())?;
    }
    Ok(0)
}

fn run_schema(py: Python<'_>, recall: &RecallRequest) -> PyResult<i32> {
    let (project_dir, settings) = settings(py, recall)?;
    if recall.ddl {
        if recall.backend.as_deref() == Some(WAREHOUSE_BACKEND) {
            return Err(user_error(
                py,
                "--ddl prints the Recall store's DDL and cannot be combined with --backend \
                 warehouse",
                RECALL_QUERY_CODE,
            ));
        }
        return run_schema_ddl(py, &settings);
    }
    if recall.backend.as_deref() == Some(WAREHOUSE_BACKEND) {
        let sql = match &recall.relation {
            Some(relation) => format!(
                "SELECT table_schema, table_name, column_name, data_type, is_nullable \
                 FROM information_schema.columns WHERE table_name = '{}' \
                 ORDER BY table_schema, ordinal_position",
                relation.replace('\'', "''")
            ),
            None => "SELECT table_schema, table_name, table_type FROM information_schema.tables \
                     WHERE table_name LIKE '\\_sqlbuild\\_%' ESCAPE '\\' \
                     ORDER BY table_schema, table_name"
                .to_owned(),
        };
        return warehouse_query(py, recall, &project_dir, (&sql, None));
    }
    let store = open_store(py, &settings, true)?;
    let described = match &store {
        ReadStore::Sqlite { store, .. } => describe_store(store),
        ReadStore::ClickHouse(reader) => reader.describe(py),
    };
    let (dialect, label) = (store.dialect(), store.label());
    store.close();
    let relations = described.map_err(|error| store_failure(py, &error))?;
    let json_output: bool = recall.format.as_deref() == Some(JSON_FORMAT);
    write_stdout_bytes(
        py,
        render_schema(
            &relations,
            json_output,
            (&label, dialect, settings.retention_days),
        )
        .as_bytes(),
    )?;
    Ok(0)
}

fn tail_lines(data: &[u8], lines: usize) -> &[u8] {
    let body = data.strip_suffix(b"\n").unwrap_or(data);
    let start = body
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, byte)| **byte == b'\n')
        .nth(lines.saturating_sub(1))
        .map_or(0, |(index, _)| index + 1);
    if lines == 0 {
        &data[data.len()..]
    } else {
        &data[start..]
    }
}

fn run_output(py: Python<'_>, recall: &RecallRequest) -> PyResult<i32> {
    let (_, settings) = settings(py, recall)?;
    let store = open_store(py, &settings, true)?;
    let selector: &str = recall.invocation.as_deref().unwrap_or(LAST_INVOCATION);
    let stream = recall.stream.as_deref().and_then(OutputStream::from_name);
    let read = store
        .resolve_invocation(py, selector)
        .and_then(|invocation_id| match &store {
            ReadStore::Sqlite { store, .. } => {
                read_output_transcript(store, &invocation_id, stream)
            }
            ReadStore::ClickHouse(reader) => {
                reader.read_output_transcript(py, &invocation_id, stream)
            }
        });
    store.close();
    let transcript = read.map_err(|error| store_failure(py, &error))?;
    let invocation_id: &str = &transcript.invocation_id;
    let data: &[u8] = match recall.tail {
        Some(lines) => tail_lines(&transcript.data, usize::try_from(lines).unwrap_or(0)),
        None => &transcript.data,
    };
    write_stdout_bytes(py, data)?;
    let mut problems: Vec<String> = Vec::new();
    if transcript.missing_sequences > 0 {
        problems.push(format!(
            "{} chunk(s) missing ({})",
            transcript.missing_sequences,
            transcript.gap_reason.as_deref().unwrap_or("unexplained")
        ));
    }
    if transcript.reported_drops > 0 {
        problems.push(format!("{} chunk(s) dropped", transcript.reported_drops));
    }
    if transcript.filtered > 0 {
        problems.push(format!("{} chunk(s) filtered", transcript.filtered));
    }
    if !transcript.flush_complete {
        problems.push("delivery did not report a complete flush".to_owned());
    }
    match transcript.status.as_deref() {
        Some("completed" | "failed" | "cancelled") | None => {}
        Some(status) => problems.push(format!("the invocation is {status}")),
    }
    if !problems.is_empty() {
        write_stderr(
            py,
            &format!(
                "note: output of {} may be incomplete: {}\n",
                short_invocation_id(invocation_id),
                problems.join("; ")
            ),
        )?;
    }
    Ok(0)
}

/// Run one `sqb recall` subcommand and return its exit code.
pub(crate) fn run_recall_command(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<i32> {
    let recall: RecallRequest = recall_request(py, options)?;
    match recall.command.as_deref() {
        Some("query") => run_query(py, &recall),
        Some("schema") => run_schema(py, &recall),
        Some("output") => run_output(py, &recall),
        _ => Err(user_error(
            py,
            "sqb recall requires a subcommand: query, schema or output",
            RECALL_QUERY_CODE,
        )),
    }
}
