//! Read-only warehouse adapter edge; never compile declarations or write warehouse state.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use serde_json::{Value, json};
use sqlbuild_analysis::fingerprint_storage::constants::{COLUMN_TIMESTAMP, FINGERPRINT_COLUMNS};
use sqlbuild_config::command_connection::constants::{
    DUCKDB_ADAPTER, DUCKDB_MEMORY_DATABASE, LOCAL_DATABASE_PATH_KEYS,
};
use sqlbuild_core::constants::{FINGERPRINT_TABLE_NAME, SOURCE_FRESHNESS_TABLE_NAME};
use sqlbuild_core::models::SqlValue;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryPage, WarehouseHistoryRead, WarehouseHistoryRecord,
};
use sqlbuild_state::freshness::constants::{
    OBSERVED_AT_COLUMN, SOURCE_FRESHNESS_COLUMN_TYPES, SQLITE_ADAPTER,
};
use sqlbuild_state::state_tables::main::read_history_sql::read_history_sql;
use sqlbuild_state::state_tables::models::HistoryRead;

use crate::bindings::_helpers::authored_values::sql_values::{SqlValueClasses, sql_value};
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::fingerprints::latest_rows::row_fingerprint;
use crate::bindings::_helpers::freshness::state_io::row_record;
use crate::bindings::_helpers::freshness::support::presumed_utc_datetime;
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
        WarehouseHistoryRead::SourceFreshness(read) => &read.location,
        WarehouseHistoryRead::Fingerprints(read) => &read.location,
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
    let (location, after, since, table, columns, timestamp) = match request {
        WarehouseHistoryRead::SourceFreshness(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            SOURCE_FRESHNESS_TABLE_NAME,
            freshness_columns.as_slice(),
            OBSERVED_AT_COLUMN,
        ),
        WarehouseHistoryRead::Fingerprints(read) => (
            &read.location,
            read.after.as_ref().map(|cursor| cursor.as_slice()),
            read.observed_since,
            FINGERPRINT_TABLE_NAME,
            FINGERPRINT_COLUMNS.as_slice(),
            COLUMN_TIMESTAMP,
        ),
    };
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
    let mut has_event_id = false;
    for column in catalog_columns.try_iter()? {
        has_event_id |= field(&column?, "name")?
            .extract::<String>()?
            .eq_ignore_ascii_case("event_id");
    }
    let mut literals = vec![None; columns.len() + 1];
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
    let sql = read_history_sql(
        &qualified,
        after.map(|_| literals.as_slice()),
        HistoryRead {
            columns,
            timestamp_column: timestamp,
            observed_since: observed_since.as_deref(),
            adapter: &adapter_name,
            has_event_id,
            limit: MAX_HISTORY_RECORDS,
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
                pyo3::IntoPyObjectExt::into_bound_py_any(MAX_HISTORY_RECORDS, py)?,
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
        let mut cursor = vec![None; columns.len() + 1];
        for (index, value) in cursor.iter_mut().enumerate() {
            let raw = row.get_item(index)?;
            *value = if raw.is_none() {
                None
            } else {
                Some(raw.str()?.extract()?)
            };
        }
        let observed = match &record {
            WarehouseHistoryRecord::SourceFreshness(record) => record.observed_at,
            WarehouseHistoryRecord::Fingerprint(record) => record.ts,
        };
        if since.is_none_or(|since| observed >= since) {
            records.push(record);
        }
        next_after = Some(cursor);
    }
    if read_count < MAX_HISTORY_RECORDS {
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
    match request {
        WarehouseHistoryRead::SourceFreshness(_) => {
            serde_json::from_value(value).map(WarehouseHistoryRecord::SourceFreshness)
        }
        WarehouseHistoryRead::Fingerprints(_) => {
            serde_json::from_value(value).map(WarehouseHistoryRecord::Fingerprint)
        }
    }
    .map_err(|error| PyValueError::new_err(error.to_string()))
}
