//! Read-only warehouse adapter edge; never compile declarations or write warehouse state.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use serde_json::{Value, json};
use sqlbuild_config::command_connection::constants::{
    DUCKDB_ADAPTER, DUCKDB_MEMORY_DATABASE, LOCAL_DATABASE_PATH_KEYS,
};
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME;
use sqlbuild_core::models::SqlValue;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_FRESHNESS_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    SourceFreshnessPage, SourceFreshnessRead, WarehouseFreshnessRecord,
};
use sqlbuild_state::freshness::constants::{
    OBSERVED_AT_COLUMN, SOURCE_FRESHNESS_COLUMN_TYPES, SQLITE_ADAPTER,
};
use sqlbuild_state::freshness::main::read_history_sql::read_history_sql;

use crate::bindings::_helpers::authored_values::sql_values::{SqlValueClasses, sql_value};
use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::freshness::state_io::{qualified_table_name, row_record};
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use crate::bindings::_helpers::records::fields::field;

pub(crate) fn read_source_freshness(
    py: Python<'_>,
    arguments: &KeywordArguments<'_>,
    request: &SourceFreshnessRead,
) -> PyResult<SourceFreshnessPage> {
    if request.location.schema.trim().is_empty() {
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
    request: &SourceFreshnessRead,
) -> PyResult<SourceFreshnessPage> {
    let database = request.location.database.as_ref().map_or_else(
        || py.None().into_bound(py),
        |value| PyString::new(py, value).into_any(),
    );
    let schema = PyString::new(py, &request.location.schema).into_any();
    let name = PyString::new(py, SOURCE_FRESHNESS_TABLE_NAME).into_any();
    let exists = call_with(
        &adapter.getattr("relation_exists")?,
        &[
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", name),
        ],
    )?
    .is_truthy()?;
    if !exists {
        return Ok(SourceFreshnessPage {
            records: Vec::new(),
            next_after: None,
        });
    }
    let columns = call_with(
        &adapter.getattr("get_columns")?,
        &[
            ("connection", connection.clone()),
            ("database", database.clone()),
            ("schema", schema.clone()),
            (
                "name",
                PyString::new(py, SOURCE_FRESHNESS_TABLE_NAME).into_any(),
            ),
        ],
    )?;
    let mut has_event_id = false;
    for column in columns.try_iter()? {
        has_event_id |= field(&column?, "name")?
            .extract::<String>()?
            .eq_ignore_ascii_case("event_id");
    }
    let mut literals: [Option<String>; 11] = Default::default();
    if let Some(after) = &request.after {
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
    let qualified = qualified_table_name(
        &adapter.getattr("render_qualified_name")?,
        (&database, &schema),
    )?;
    let adapter_name = adapter.getattr("adapter_name")?.extract::<String>()?;
    let observed_since = request
        .observed_since
        .map(|since| {
            let text = since
                .format(if adapter_name == SQLITE_ADAPTER {
                    "%Y-%m-%d"
                } else {
                    "%Y-%m-%dT%H:%M:%S+00:00"
                })
                .to_string();
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
        request.after.as_ref().map(|_| &literals),
        observed_since.as_deref(),
        (MAX_FRESHNESS_RECORDS, &adapter_name, has_event_id),
    )
    .map_err(PyValueError::new_err)?;
    let result = call_with(
        &adapter.getattr("query")?,
        &[
            ("connection", connection.clone()),
            ("sql", PyString::new(py, &sql).into_any()),
            (
                "limit",
                pyo3::IntoPyObjectExt::into_bound_py_any(MAX_FRESHNESS_RECORDS, py)?,
            ),
        ],
    )?;
    let mut records: Vec<WarehouseFreshnessRecord> = Vec::new();
    let mut next_after = None;
    let mut read_count = 0;
    for row in field(&result, "rows")?.try_iter()? {
        read_count += 1;
        let row = row?;
        let record = row_record(&row)?;
        let mut cursor: [Option<String>; 11] = Default::default();
        let mut value = json!({"state_database": request.location.database, "state_schema": request.location.schema});
        for (index, (name, _)) in SOURCE_FRESHNESS_COLUMN_TYPES.iter().enumerate() {
            let raw = row.get_item(index)?;
            cursor[index] = if raw.is_none() {
                None
            } else {
                Some(raw.str()?.extract()?)
            };
            let item = field(&record, name)?;
            value[*name] = if item.is_none() {
                Value::Null
            } else if *name == OBSERVED_AT_COLUMN {
                json!(item.call_method0("isoformat")?.extract::<String>()?)
            } else {
                json!(item.extract::<String>()?)
            };
        }
        cursor[10] = row.get_item(10)?.extract::<Option<String>>()?;
        value["event_id"] = json!(cursor[10]);
        let record = serde_json::from_value::<WarehouseFreshnessRecord>(value)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        if request
            .observed_since
            .is_none_or(|since| record.observed_at >= since)
        {
            records.push(record);
        }
        next_after = Some(cursor);
    }
    if read_count < MAX_FRESHNESS_RECORDS {
        next_after = None;
    }
    Ok(SourceFreshnessPage {
        records,
        next_after,
    })
}
