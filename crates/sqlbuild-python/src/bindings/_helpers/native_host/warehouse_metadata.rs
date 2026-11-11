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
    COLUMN_MIGRATION_TABLE_NAME, CONSUMPTION_TABLE_NAME, FINGERPRINT_TABLE_NAME,
    NODE_RESULTS_TABLE_NAME, SOURCE_FRESHNESS_TABLE_NAME,
};
use sqlbuild_core::models::SqlValue;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryPage, WarehouseHistoryRead, WarehouseHistoryRecord, WarehouseStateLocation,
};
use sqlbuild_state::consumption::constants::{CONSUMPTION_COLUMN_TYPES, RECORDED_AT_COLUMN};
use sqlbuild_state::events::constants::{
    COLUMN_MIGRATION_COLUMNS, MIGRATION_COLUMNS, NODE_RESULT_COLUMNS, OLD_NAME_VIEW_COLUMNS,
};
use sqlbuild_state::freshness::constants::{
    OBSERVED_AT_COLUMN, SOURCE_FRESHNESS_COLUMN_TYPES, SQLITE_ADAPTER,
};
use sqlbuild_state::state_tables::main::read_history_sql::read_history_sql;
use sqlbuild_state::state_tables::models::HistoryRead;

use crate::bindings::_helpers::authored_values::sql_values::{SqlValueClasses, sql_value};
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::fingerprints::latest_rows::row_fingerprint;
use crate::bindings::_helpers::freshness::state_io::row_record;
use crate::bindings::_helpers::freshness::support::presumed_utc_datetime;
use crate::bindings::_helpers::native_host::warehouse_catalog::read_catalog;
use crate::bindings::_helpers::native_host::warehouse_statistics::read_statistics;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::{construct::record_class, ids::RecordId};

pub(crate) fn read_history(
    py: Python<'_>,
    arguments: &KeywordArguments<'_>,
    request: &WarehouseHistoryRead,
) -> PyResult<WarehouseHistoryPage> {
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
        | WarehouseHistoryRead::ColumnMigrations(read) => &read.location,
        WarehouseHistoryRead::NodeResults(read) => &read.location,
        WarehouseHistoryRead::Consumption(read) => &read.location,
    };
    if location.schema.trim().is_empty() {
        return Err(PyValueError::new_err(
            "metadata state schema must not be blank",
        ));
    }
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
    if let Some((_, path_key)) = LOCAL_DATABASE_PATH_KEYS
        .iter()
        .find(|(name, _)| *name == adapter_name)
    {
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
    let read = read_page(py, (&adapter, &connection), request);
    let closed = adapter.call_method1("close", (&connection,));
    let page = read?;
    closed?;
    Ok(page)
}

fn read_page(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    request: &WarehouseHistoryRead,
) -> PyResult<WarehouseHistoryPage> {
    let freshness_columns = SOURCE_FRESHNESS_COLUMN_TYPES.map(|(name, _)| name);
    let consumption_columns = CONSUMPTION_COLUMN_TYPES.map(|(name, _)| name);
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
            return read_statistics(py, (adapter, connection), read);
        }
        WarehouseHistoryRead::Catalog(read) => {
            return read_catalog(py, (adapter, connection), read);
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
            "_sqlbuild_audit_results",
            event_columns.as_slice(),
            "occurred_at",
        ),
        WarehouseHistoryRead::JanitorEvents(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            "_sqlbuild_janitor_events",
            event_columns.as_slice(),
            "occurred_at",
        ),
        WarehouseHistoryRead::Migrations(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            "_sqlbuild_migrations",
            MIGRATION_COLUMNS.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::OldNameViews(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            "_sqlbuild_old_name_views",
            OLD_NAME_VIEW_COLUMNS.as_slice(),
            "created_at",
        ),
        WarehouseHistoryRead::Microbatches(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            read.limit,
            "_sqlbuild_microbatches",
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
    };
    let limit = limit.unwrap_or(MAX_HISTORY_RECORDS);
    if limit == 0 || limit > MAX_HISTORY_RECORDS {
        return Err(PyValueError::new_err(
            "metadata page limit is outside the supported bounds",
        ));
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
        return Ok(WarehouseHistoryPage {
            records: Vec::new(),
            next_after: None,
        });
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
            limit,
        },
    )
    .map_err(PyValueError::new_err)?;
    let result = call_with(
        &adapter.getattr("query")?,
        &[
            ("connection", connection.clone()),
            ("sql", PyString::new(py, &sql).into_any()),
            (
                "limit",
                pyo3::IntoPyObjectExt::into_bound_py_any(limit, py)?,
            ),
        ],
    )?;
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    let mut next_after = None;
    let mut read_count = 0;
    for row in field(&result, "rows")?.try_iter()? {
        read_count += 1;
        let row = row?;
        let record = decode_record(py, (request, &row, &qualified), (columns, timestamp))?;
        let mut cursor = vec![None; cursor_indexes.len()];
        for (index, value) in cursor.iter_mut().enumerate() {
            let raw = row.get_item(cursor_indexes[index])?;
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
        };
        if since.is_none_or(|since| observed >= since) {
            records.push(record);
        }
        next_after = Some(cursor);
    }
    if read_count < limit {
        next_after = None;
    }
    Ok(WarehouseHistoryPage {
        records,
        next_after,
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
        | WarehouseHistoryRead::ColumnMigrations(read) => {
            return decode_event_record(&read.location, row, (columns, &[timestamp]));
        }
        WarehouseHistoryRead::NodeResults(read) => {
            return decode_event_record(&read.location, row, (columns, &[timestamp]));
        }
        WarehouseHistoryRead::Consumption(read) => {
            return decode_event_record(&read.location, row, (columns, &[timestamp]));
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
