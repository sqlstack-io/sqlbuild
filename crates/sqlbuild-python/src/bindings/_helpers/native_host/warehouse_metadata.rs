//! Read-only warehouse adapter edge; never compile declarations or write warehouse state.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use serde_json::{Value, json};
use sqlbuild_analysis::fingerprint_storage::constants::{COLUMN_TIMESTAMP, FINGERPRINT_COLUMNS};
use sqlbuild_config::command_connection::constants::{
    DUCKDB_ADAPTER, DUCKDB_MEMORY_DATABASE, LOCAL_DATABASE_PATH_KEYS,
};
use sqlbuild_core::constants::{
    AUDIT_RESULTS_TABLE_NAME, COLUMN_MIGRATION_TABLE_NAME, CONSUMPTION_TABLE_NAME,
    FINGERPRINT_TABLE_NAME, JANITOR_EVENTS_TABLE_NAME, MICROBATCH_TABLE_NAME, MIGRATION_TABLE_NAME,
    NODE_RESULTS_TABLE_NAME, OBSERVATIONS_TABLE_NAME, OLD_NAME_VIEW_TABLE_NAME,
    RESOURCE_COSTS_TABLE_NAME, SOURCE_FRESHNESS_TABLE_NAME,
};
use sqlbuild_core::models::SqlValue;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    ConsumptionRead, NodeResultsRead, WarehouseHistoryPage, WarehouseHistoryRead,
    WarehouseHistoryRecord, WarehouseStateLocation,
};
use sqlbuild_state::consumption::constants::{CONSUMPTION_COLUMN_TYPES, RECORDED_AT_COLUMN};
use sqlbuild_state::events::constants::{
    COLUMN_MIGRATION_COLUMNS, MIGRATION_COLUMNS, NODE_RESULT_COLUMNS, OLD_NAME_VIEW_COLUMNS,
};
use sqlbuild_state::freshness::constants::{
    OBSERVED_AT_COLUMN, SOURCE_FRESHNESS_COLUMN_TYPES, SQLITE_ADAPTER,
};
use sqlbuild_state::observations::constants::OBSERVATION_COLUMN_TYPES;
use sqlbuild_state::resource_costs::constants::RESOURCE_COST_COLUMN_TYPES;
use sqlbuild_state::state_tables::main::read_history_sql::read_history_sql;
use sqlbuild_state::state_tables::models::HistoryRead;

use crate::bindings::_helpers::authored_values::sql_values::{SqlValueClasses, sql_value};
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::fingerprints::latest_rows::row_fingerprint;
use crate::bindings::_helpers::freshness::state_io::row_record;
use crate::bindings::_helpers::freshness::support::presumed_utc_datetime;
use crate::bindings::_helpers::native_host::constants::HISTORY_STREAM_PAGES;
use crate::bindings::_helpers::native_host::warehouse_catalog::read_catalog;
use crate::bindings::_helpers::native_host::warehouse_statistics::read_statistics;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::{construct::record_class, ids::RecordId};

/// One read-only adapter session serves every page of one synchronization.
pub(crate) struct HistorySession<'py> {
    adapter: Bound<'py, PyAny>,
    connection: Bound<'py, PyAny>,
    /// A local database file, whose read-only connection would block other processes' writers.
    pub(crate) local: bool,
}

/// One ordered SELECT per table scan; each page fetches the next rows of the same result.
pub(crate) struct HistoryStream<'py> {
    /// The request that opened the stream, without its cursor, limit or token.
    scan: Value,
    /// A DB-API cursor, or DuckDB's connection; dropped, never closed, when discarded.
    cursor: Bound<'py, PyAny>,
    /// The cursor the last delivered page ended at; only a request for exactly it continues.
    next_after: Vec<Option<String>>,
    /// Rows the bounded SELECT can still return; at zero the next page reads from its cursor.
    remaining: usize,
    qualified: String,
    columns: Vec<String>,
    timestamp: String,
    cursor_indexes: Vec<usize>,
    since: Option<chrono::DateTime<chrono::Utc>>,
}

pub(crate) fn open_history_session<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
) -> PyResult<HistorySession<'py>> {
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::Configuration,
    )?;
    let connection_config = project_connection_config(
        &discovered,
        (&project_dir, &arguments.value("selected_target")?),
        (&py.None().into_bound(py), true),
        true,
    )?;
    let adapter_name: String = adapter.getattr("adapter_name")?.extract()?;
    let local_path_key = LOCAL_DATABASE_PATH_KEYS
        .iter()
        .find(|(name, _)| *name == adapter_name);
    if let Some((_, path_key)) = local_path_key {
        let config = connection_config.cast::<PyDict>()?;
        let path: Option<String> = config
            .get_item(path_key)?
            .map(|path| path.extract())
            .transpose()?;
        if path
            .as_deref()
            .is_some_and(|path| path != DUCKDB_MEMORY_DATABASE)
        {
            config.set_item("read_only", true)?;
        }
        if adapter_name == DUCKDB_ADAPTER
            && let Some(attachments) = config.get_item("attach")?
        {
            for attachment in attachments.try_iter()? {
                attachment?.set_item("read_only", true)?;
            }
        }
    }
    let connection = adapter.call_method1("connect", (connection_config,))?;
    Ok(HistorySession {
        adapter,
        connection,
        local: local_path_key.is_some(),
    })
}

pub(crate) fn close_history_session(
    session: HistorySession<'_>,
    stream: Option<HistoryStream<'_>>,
) -> PyResult<()> {
    discard_stream(&session.connection, stream)?;
    session
        .adapter
        .call_method1("close", (&session.connection,))?;
    Ok(())
}

/// Returns the page and the scan's still-open stream, which only its next page continues.
pub(crate) fn read_history<'py>(
    py: Python<'py>,
    (session, stream): (&HistorySession<'py>, Option<HistoryStream<'py>>),
    request: &WarehouseHistoryRead,
) -> PyResult<(WarehouseHistoryPage, Option<HistoryStream<'py>>)> {
    let location = match request {
        WarehouseHistoryRead::Statistics(read) => &read.location,
        WarehouseHistoryRead::Catalog(read) => &read.location,
        WarehouseHistoryRead::SourceFreshness(read) => &read.location,
        WarehouseHistoryRead::Fingerprints(read) => &read.location,
        WarehouseHistoryRead::AuditResults(read)
        | WarehouseHistoryRead::JanitorEvents(read)
        | WarehouseHistoryRead::Migrations(read)
        | WarehouseHistoryRead::OldNameViews(read)
        | WarehouseHistoryRead::Microbatches(read)
        | WarehouseHistoryRead::ColumnMigrations(read)
        | WarehouseHistoryRead::Observations(read)
        | WarehouseHistoryRead::ResourceCosts(read) => &read.location,
        WarehouseHistoryRead::NodeResults(read) => &read.location,
        WarehouseHistoryRead::Consumption(read) => &read.location,
    };
    if location.schema.trim().is_empty() {
        return Err(PyValueError::new_err(
            "metadata state schema must not be blank",
        ));
    }
    read_page(py, (session, stream), request)
}

/// Any request but the open scan's next page discards that result before running other SQL.
fn read_page<'py>(
    py: Python<'py>,
    (session, stream): (&HistorySession<'py>, Option<HistoryStream<'py>>),
    request: &WarehouseHistoryRead,
) -> PyResult<(WarehouseHistoryPage, Option<HistoryStream<'py>>)> {
    let (scan, after_value) = scan_identity(request)?;
    let continued = match stream {
        Some(stream)
            if stream.scan == scan && after_value.as_ref() == Some(&json!(stream.next_after)) =>
        {
            Some(stream)
        }
        other => {
            discard_stream(&session.connection, other)?;
            None
        }
    };
    let (adapter, connection) = (&session.adapter, &session.connection);
    let freshness_columns = SOURCE_FRESHNESS_COLUMN_TYPES.map(|(name, _)| name);
    let consumption_columns = CONSUMPTION_COLUMN_TYPES.map(|(name, _)| name);
    let observation_columns = OBSERVATION_COLUMN_TYPES.map(|(name, _)| name);
    let resource_cost_columns = RESOURCE_COST_COLUMN_TYPES.map(|(name, _)| name);
    let event_columns: Vec<String> = match request {
        WarehouseHistoryRead::AuditResults(_) => py
            .import("sqlbuild.executor.audit_results.constants")?
            .getattr("AUDIT_RESULT_COLUMNS")?
            .extract()?,
        WarehouseHistoryRead::JanitorEvents(_) => py
            .import("sqlbuild.executor.janitor_events.constants")?
            .getattr("JANITOR_EVENT_COLUMNS")?
            .extract()?,
        WarehouseHistoryRead::Microbatches(_) => py
            .import("sqlbuild.microbatches.constants")?
            .getattr("MICROBATCH_COLUMNS")?
            .extract()?,
        _ => Vec::new(),
    };
    let event_columns: Vec<&str> = event_columns.iter().map(String::as_str).collect();
    let (location, after, since, limit, table, columns, timestamp) = match request {
        WarehouseHistoryRead::Statistics(read) => {
            return Ok((read_statistics(py, (adapter, connection), read)?, None));
        }
        WarehouseHistoryRead::Catalog(read) => {
            return Ok((read_catalog(py, (adapter, connection), read)?, None));
        }
        WarehouseHistoryRead::SourceFreshness(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            SOURCE_FRESHNESS_TABLE_NAME,
            freshness_columns.as_slice(),
            OBSERVED_AT_COLUMN,
        ),
        WarehouseHistoryRead::Fingerprints(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            FINGERPRINT_TABLE_NAME,
            FINGERPRINT_COLUMNS.as_slice(),
            COLUMN_TIMESTAMP,
        ),
        WarehouseHistoryRead::AuditResults(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            AUDIT_RESULTS_TABLE_NAME,
            event_columns.as_slice(),
            "occurred_at",
        ),
        WarehouseHistoryRead::JanitorEvents(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            JANITOR_EVENTS_TABLE_NAME,
            event_columns.as_slice(),
            "occurred_at",
        ),
        WarehouseHistoryRead::Migrations(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            MIGRATION_TABLE_NAME,
            MIGRATION_COLUMNS.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::OldNameViews(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            OLD_NAME_VIEW_TABLE_NAME,
            OLD_NAME_VIEW_COLUMNS.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::Microbatches(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            MICROBATCH_TABLE_NAME,
            event_columns.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::ColumnMigrations(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            COLUMN_MIGRATION_TABLE_NAME,
            COLUMN_MIGRATION_COLUMNS.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::NodeResults(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            NODE_RESULTS_TABLE_NAME,
            NODE_RESULT_COLUMNS.as_slice(),
            "ts",
        ),
        WarehouseHistoryRead::Consumption(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            CONSUMPTION_TABLE_NAME,
            consumption_columns.as_slice(),
            RECORDED_AT_COLUMN,
        ),
        WarehouseHistoryRead::Observations(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            OBSERVATIONS_TABLE_NAME,
            observation_columns.as_slice(),
            OBSERVED_AT_COLUMN,
        ),
        WarehouseHistoryRead::ResourceCosts(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            RESOURCE_COSTS_TABLE_NAME,
            resource_cost_columns.as_slice(),
            OBSERVED_AT_COLUMN,
        ),
    };
    let limit = limit.unwrap_or(MAX_HISTORY_RECORDS);
    if limit == 0 || limit > MAX_HISTORY_RECORDS {
        return Err(PyValueError::new_err(
            "metadata page limit is outside the supported bounds",
        ));
    }
    if let Some(stream) = continued {
        return stream_page(py, (session, request), stream, limit);
    }
    let database = location.database.as_ref().map_or_else(
        || py.None().into_bound(py),
        |value| PyString::new(py, value).into_any(),
    );
    let schema = PyString::new(py, &location.schema).into_any();
    let name = PyString::new(py, table).into_any();
    let exists = call_with(
        &adapter.getattr("relation_exists")?,
        &[
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name.clone()),
        ],
    )?
    .is_truthy()?;
    if !exists {
        let page = WarehouseHistoryPage {
            records: Vec::new(),
            next_after: None,
            change_token: None,
            unchanged: false,
        };
        return Ok((page, None));
    }
    let change_token: Option<String> = if after.is_none() {
        relation_change_token(py, (adapter, connection), (&database, &schema, &name))?
    } else {
        None
    };
    if change_token.is_some() && change_token.as_deref() == unchanged_token(request) {
        let page = WarehouseHistoryPage {
            records: Vec::new(),
            next_after: None,
            change_token,
            unchanged: true,
        };
        return Ok((page, None));
    }
    let catalog_columns = call_with(
        &adapter.getattr("get_columns")?,
        &[
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name.clone()),
        ],
    )?;
    let mut actual_columns: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for column in catalog_columns.try_iter()? {
        actual_columns.insert(field(&column?, "name")?.extract::<String>()?.to_lowercase());
    }
    let has_event_id = actual_columns.contains("event_id");
    let optional_columns: &[&str] = match request {
        WarehouseHistoryRead::AuditResults(_) => &["audit_description", "audit_meta_json"],
        WarehouseHistoryRead::Migrations(_) => &[
            "target_name",
            "origin_model",
            "origin_database",
            "origin_schema",
            "destination_database",
        ],
        WarehouseHistoryRead::OldNameViews(_) => &[
            "target_name",
            "old_database",
            "old_schema",
            "new_database",
            "view_retention",
            "archive_name",
            "column_aliases",
            "grants_copied",
            "view_sql",
            "expires_at",
            "drop_reason",
        ],
        _ => &[],
    };
    let key_columns: Option<&[&str]> = match request {
        WarehouseHistoryRead::AuditResults(_) => Some(&["result_id", "occurred_at"]),
        WarehouseHistoryRead::JanitorEvents(_) => Some(&["event_id", "occurred_at"]),
        WarehouseHistoryRead::Migrations(_)
        | WarehouseHistoryRead::OldNameViews(_)
        | WarehouseHistoryRead::Microbatches(_)
        | WarehouseHistoryRead::ColumnMigrations(_) => Some(&["event_id", "created_at"]),
        WarehouseHistoryRead::NodeResults(_) => Some(&[
            "node_type",
            "node_name",
            "target_database",
            "target_schema",
            "target_name",
            "run_id",
            "status",
            "ts",
        ]),
        WarehouseHistoryRead::Consumption(_) => Some(&[
            "execution_id",
            "phase",
            "input_kind",
            "input_name",
            "input_version",
            RECORDED_AT_COLUMN,
        ]),
        WarehouseHistoryRead::Observations(_) | WarehouseHistoryRead::ResourceCosts(_) => {
            Some(&["observation_id", OBSERVED_AT_COLUMN])
        }
        _ => None,
    };
    let mut cursor_indexes: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            key_columns
                .is_none_or(|keys| keys.contains(name))
                .then_some(index)
        })
        .collect();
    cursor_indexes.push(columns.len());
    let mut literals = vec![None; key_columns.unwrap_or(columns).len() + 1];
    if let Some(after) = after {
        let classes = SqlValueClasses::load(py)?;
        for (output, value) in literals.iter_mut().zip(after) {
            if let Some(value) = value {
                let value = sql_value(&SqlValue::String(value.clone()), &classes)?;
                *output = Some(
                    call_with(
                        &adapter.getattr("render_typed_scalar")?,
                        &[("value", value)],
                    )?
                    .extract()?,
                );
            }
        }
    }
    let qualified: String = call_with(
        &adapter.getattr("render_qualified_name")?,
        &[("database", database), ("schema", schema), ("name", name)],
    )?
    .extract()?;
    let adapter_name = adapter.getattr("adapter_name")?.extract::<String>()?;
    let observed_since = since
        .map(|since| {
            let text = if adapter_name == SQLITE_ADAPTER {
                since
                    .date_naive()
                    .pred_opt()
                    .unwrap_or(since.date_naive())
                    .format("%Y-%m-%d")
                    .to_string()
            } else {
                since.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
            };
            let classes = SqlValueClasses::load(py)?;
            let value = sql_value(&SqlValue::String(text), &classes)?;
            call_with(
                &adapter.getattr("render_typed_scalar")?,
                &[("value", value)],
            )?
            .extract::<String>()
        })
        .transpose()?;
    let projection: Vec<String> = columns
        .iter()
        .map(|column| {
            if optional_columns.contains(column) && !actual_columns.contains(*column) {
                format!("NULL AS {column}")
            } else {
                (*column).to_owned()
            }
        })
        .collect();
    let projection: Vec<&str> = projection.iter().map(String::as_str).collect();
    let window = if session.local {
        limit
    } else {
        limit * HISTORY_STREAM_PAGES
    };
    let sql = read_history_sql(
        &qualified,
        after.map(|_| literals.as_slice()),
        HistoryRead {
            columns: &projection,
            key_columns: key_columns.or(Some(columns)),
            timestamp_column: timestamp,
            observed_since: observed_since.as_deref(),
            adapter: &adapter_name,
            has_event_id,
            limit: window,
        },
    )
    .map_err(PyValueError::new_err)?;
    let cursor = call_with(
        &adapter.getattr("execute")?,
        &[
            ("connection", connection.clone()),
            ("sql", PyString::new(py, &sql).into_any()),
        ],
    )?;
    let stream = HistoryStream {
        scan,
        cursor,
        next_after: Vec::new(),
        remaining: window,
        qualified,
        columns: columns.iter().map(|column| (*column).to_owned()).collect(),
        timestamp: timestamp.to_owned(),
        cursor_indexes,
        since,
    };
    let (mut page, stream) = stream_page(py, (session, request), stream, limit)?;
    page.change_token = change_token;
    Ok((page, stream))
}

/// Fetch one page from the scan's result; a full page keeps the stream open for the next.
fn stream_page<'py>(
    py: Python<'py>,
    (session, request): (&HistorySession<'py>, &WarehouseHistoryRead),
    mut stream: HistoryStream<'py>,
    limit: usize,
) -> PyResult<(WarehouseHistoryPage, Option<HistoryStream<'py>>)> {
    let columns: Vec<&str> = stream.columns.iter().map(String::as_str).collect();
    let tuple = py.import("builtins")?.getattr("tuple")?;
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    let mut next_after = None;
    let mut read_count = 0;
    let fetch = limit.min(stream.remaining);
    for row in stream
        .cursor
        .call_method1("fetchmany", (fetch,))?
        .try_iter()?
    {
        read_count += 1;
        let row = tuple.call1((row?,))?;
        let record = decode_record(
            py,
            (request, &row, &stream.qualified),
            (&columns, &stream.timestamp),
        )?;
        let mut cursor = vec![None; stream.cursor_indexes.len()];
        for (index, value) in cursor.iter_mut().enumerate() {
            let raw = row.get_item(stream.cursor_indexes[index])?;
            *value = if raw.is_none() {
                None
            } else {
                Some(raw.str()?.extract()?)
            };
        }
        let observed = match &record {
            WarehouseHistoryRecord::Catalog(_) | WarehouseHistoryRecord::Statistics(_) => {
                return Err(PyValueError::new_err(
                    "catalog does not use timestamp history filtering",
                ));
            }
            WarehouseHistoryRecord::SourceFreshness(record) => record.observed_at,
            WarehouseHistoryRecord::Fingerprint(record) => record.ts,
            WarehouseHistoryRecord::AuditResult(record) => record.occurred_at,
            WarehouseHistoryRecord::JanitorEvent(record) => record.occurred_at,
            WarehouseHistoryRecord::Migration(record) => record.created_at,
            WarehouseHistoryRecord::OldNameView(record) => record.created_at,
            WarehouseHistoryRecord::Microbatch(record) => record.created_at,
            WarehouseHistoryRecord::ColumnMigration(record) => record.created_at,
            WarehouseHistoryRecord::NodeResult(record) => record.ts,
            WarehouseHistoryRecord::Consumption(record) => record.recorded_at,
            WarehouseHistoryRecord::Observation(record) => record.observed_at,
            WarehouseHistoryRecord::ResourceCost(record) => record.observed_at,
        };
        if stream.since.is_none_or(|since| observed >= since) {
            records.push(record);
        }
        next_after = Some(cursor);
    }
    stream.remaining -= read_count;
    let open = match &next_after {
        Some(after) if read_count == fetch && stream.remaining > 0 => {
            stream.next_after.clone_from(after);
            Some(stream)
        }
        Some(_) if read_count == fetch => {
            discard_stream(&session.connection, Some(stream))?;
            None
        }
        _ => {
            next_after = None;
            discard_stream(&session.connection, Some(stream))?;
            None
        }
    };
    let page = WarehouseHistoryPage {
        records,
        next_after,
        change_token: None,
        unchanged: false,
    };
    Ok((page, open))
}

/// A scan's identity without its position, and the position, so a next page continues its result.
fn scan_identity(request: &WarehouseHistoryRead) -> PyResult<(Value, Option<Value>)> {
    let mut value =
        serde_json::to_value(request).map_err(|error| PyValueError::new_err(error.to_string()))?;
    let mut after = None;
    if let Some(read) = value
        .as_object_mut()
        .and_then(|kinds| kinds.values_mut().next())
        .and_then(Value::as_object_mut)
    {
        after = read.remove("after").filter(|after| !after.is_null());
        read.remove("limit");
        read.remove("unchanged_token");
    }
    Ok((value, after))
}

/// Free the driver statement (SQL Server allows one active result); DuckDB's is the connection.
fn discard_stream(
    connection: &Bound<'_, PyAny>,
    stream: Option<HistoryStream<'_>>,
) -> PyResult<()> {
    if let Some(stream) = stream
        && !stream.cursor.is(connection)
    {
        stream.cursor.call_method0("close")?;
    }
    Ok(())
}

/// The token a complete earlier read recorded; only state-table scans carry one.
fn unchanged_token(request: &WarehouseHistoryRead) -> Option<&str> {
    match request {
        WarehouseHistoryRead::SourceFreshness(read) => read.unchanged_token.as_deref(),
        WarehouseHistoryRead::Fingerprints(read) => read.unchanged_token.as_deref(),
        WarehouseHistoryRead::AuditResults(read)
        | WarehouseHistoryRead::JanitorEvents(read)
        | WarehouseHistoryRead::Migrations(read)
        | WarehouseHistoryRead::OldNameViews(read)
        | WarehouseHistoryRead::Microbatches(read)
        | WarehouseHistoryRead::ColumnMigrations(read)
        | WarehouseHistoryRead::Observations(read)
        | WarehouseHistoryRead::ResourceCosts(read) => read.unchanged_token.as_deref(),
        WarehouseHistoryRead::NodeResults(read) => read.unchanged_token.as_deref(),
        WarehouseHistoryRead::Consumption(read) => read.unchanged_token.as_deref(),
        WarehouseHistoryRead::Catalog(_) | WarehouseHistoryRead::Statistics(_) => None,
    }
}

/// The adapter's change token for one table; without one, or when reading it fails, the scan reads as before.
fn relation_change_token(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (database, schema, name): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Option<String>> {
    let reader = py
        .import("sqlbuild.adapter.relations.types")?
        .getattr("RelationChangeTokenReader")?;
    if !adapter.is_instance(&reader)? {
        return Ok(None);
    }
    let token = call_with(
        &adapter.getattr("read_relation_change_token")?,
        &[
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name.clone()),
        ],
    );
    Ok(match token {
        Ok(token) if !token.is_none() => Some(token.extract()?),
        _ => None,
    })
}

fn decode_record(
    py: Python<'_>,
    (request, row, qualified): (&WarehouseHistoryRead, &Bound<'_, PyAny>, &str),
    (columns, timestamp): (&[&str], &str),
) -> PyResult<WarehouseHistoryRecord> {
    let (location, record) = match request {
        WarehouseHistoryRead::Catalog(_) | WarehouseHistoryRead::Statistics(_) => {
            return Err(PyValueError::new_err(
                "catalog does not use state-table row decoding",
            ));
        }
        WarehouseHistoryRead::SourceFreshness(read) => (&read.location, row_record(row)?),
        WarehouseHistoryRead::Fingerprints(read) => (
            &read.location,
            row_fingerprint(
                &record_class(py, RecordId::Fingerprint)?,
                &py.import("datetime")?
                    .getattr("datetime")?
                    .getattr("fromisoformat")?,
                row,
                qualified,
            )?,
        ),
        WarehouseHistoryRead::AuditResults(read)
        | WarehouseHistoryRead::Migrations(read)
        | WarehouseHistoryRead::ColumnMigrations(read)
        | WarehouseHistoryRead::Observations(read) => {
            return decode_event_record(&read.location, row, (columns, &[timestamp]));
        }
        WarehouseHistoryRead::NodeResults(NodeResultsRead { location, .. })
        | WarehouseHistoryRead::Consumption(ConsumptionRead { location, .. }) => {
            return decode_event_record(location, row, (columns, &[timestamp]));
        }
        WarehouseHistoryRead::ResourceCosts(read) => {
            return decode_event_record(
                &read.location,
                row,
                (columns, &[timestamp, "run_started_at", "run_completed_at"]),
            );
        }
        WarehouseHistoryRead::JanitorEvents(read) => {
            return decode_event_record(
                &read.location,
                row,
                (columns, &[timestamp, "archived_at"]),
            );
        }
        WarehouseHistoryRead::OldNameViews(read) => {
            return decode_event_record(&read.location, row, (columns, &[timestamp, "expires_at"]));
        }
        WarehouseHistoryRead::Microbatches(read) => {
            return decode_event_record(
                &read.location,
                row,
                (
                    columns,
                    &[
                        timestamp,
                        "origin_run_started_at",
                        "execution_run_started_at",
                        "completed_at",
                        "observed_at",
                    ],
                ),
            );
        }
    };
    let mut value = json!({
        "state_database": location.database,
        "state_schema": location.schema,
        "event_id": row.get_item(columns.len())?.extract::<Option<String>>()?,
    });
    for name in columns {
        let name = match *name {
            "definition_b64" => "definition",
            "metadata_json_b64" => "metadata_json",
            name => name,
        };
        let item = field(&record, name)?;
        value[name] = if item.is_none() {
            Value::Null
        } else if name == timestamp {
            json!(
                presumed_utc_datetime(&item)?
                    .call_method0("isoformat")?
                    .extract::<String>()?
            )
        } else {
            json!(item.extract::<String>()?)
        };
    }
    if matches!(request, WarehouseHistoryRead::SourceFreshness(_)) {
        serde_json::from_value(value).map(WarehouseHistoryRecord::SourceFreshness)
    } else {
        serde_json::from_value(value).map(WarehouseHistoryRecord::Fingerprint)
    }
    .map_err(|error| PyValueError::new_err(error.to_string()))
}

fn decode_event_record(
    location: &WarehouseStateLocation,
    row: &Bound<'_, PyAny>,
    (columns, timestamps): (&[&str], &[&str]),
) -> PyResult<WarehouseHistoryRecord> {
    let mut value = json!({"state_database": location.database, "state_schema": location.schema});
    for (index, name) in columns.iter().enumerate() {
        let item = row.get_item(index)?;
        value[*name] = if timestamps.contains(name) && !item.is_none() {
            let item = if item.is_instance_of::<PyString>() {
                item.py()
                    .import("datetime")?
                    .getattr("datetime")?
                    .call_method1("fromisoformat", (&item,))?
            } else {
                item
            };
            json!(
                presumed_utc_datetime(&item)?
                    .call_method0("isoformat")?
                    .extract::<String>()?
            )
        } else {
            plain_value(&item)?
        };
    }
    serde_json::from_value(value).map_err(|error| PyValueError::new_err(error.to_string()))
}
