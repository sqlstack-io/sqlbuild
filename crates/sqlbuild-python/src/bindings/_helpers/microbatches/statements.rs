//! Direct-mode microbatch state DDL, idempotent inserts, reads and event IDs.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::MICROBATCH_TABLE_NAME;
use sqlbuild_host::entries::models::{AdapterHost, ExecutorHost};
use sqlbuild_state::events::main::microbatch_event_id::microbatch_event_id;
use sqlbuild_state::microbatches::main::create_index_sqls::create_index_sqls;
use sqlbuild_state::microbatches::main::existing_event_ids_sql::existing_event_ids_sql;
use sqlbuild_state::microbatches::main::insert_many_sql::insert_many_sql;
use sqlbuild_state::microbatches::main::read_scope_sql::read_scope_sql;
use sqlbuild_state::microbatches::models::{MicrobatchEventIdentity, ScopeRead};
use sqlbuild_state::state_tables::main::create_table_sql::create_table_sql;
use sqlbuild_state::state_tables::models::TableColumn;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::contracts::state_literals::render_state_sql_literal;
use crate::bindings::_helpers::frontier::encoder::emit_error;
use crate::bindings::_helpers::microbatches::conversion::{optional_text, state_error};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_items;

const MISSING_SCHEMA: &str = "microbatch history requires a target schema";
const EMPTY_INSERT: &str = "bulk microbatch event insert requires at least one event";
const EMPTY_LOOKUP: &str = "microbatch event lookup requires at least one event";
const INTEGER_COLUMN_TYPE: &str = "BIGINT";
const EVENT_ID_COLUMN: &str = "event_id";

/// The state table columns and each column's declared state value type.
struct Layout {
    columns: Vec<String>,
    declared: Vec<String>,
}

impl Layout {
    fn new(py: Python<'_>) -> PyResult<Self> {
        let columns: Vec<String> = host::entry(py, ExecutorHost::MicrobatchColumns)?.extract()?;
        let types: Bound<'_, PyAny> = host::entry(py, ExecutorHost::MicrobatchColumnTypes)?;
        let declared: Vec<String> = columns
            .iter()
            .map(|column| types.get_item(column)?.str()?.extract())
            .collect::<PyResult<_>>()?;
        Ok(Self { columns, declared })
    }

    fn declared(&self, column: &str) -> &str {
        self.columns
            .iter()
            .position(|known| known == column)
            .map_or("", |index| self.declared[index].as_str())
    }

    /// One event's column literals in column order, timestamps cast to `timestamp_type`.
    fn literals(
        &self,
        event: &Bound<'_, PyAny>,
        timestamp_type: Option<&str>,
    ) -> PyResult<Vec<String>> {
        let py: Python<'_> = event.py();
        let values: Bound<'_, PyAny> = host::entry(py, ExecutorHost::MicrobatchEventCodec)?
            .call_method1("values", (event,))?;
        values
            .try_iter()?
            .zip(&self.declared)
            .map(|(value, declared)| {
                render_state_sql_literal(py, &value?, declared, timestamp_type)
            })
            .collect()
    }

    fn literal(&self, column: &str, value: &Bound<'_, PyAny>) -> PyResult<String> {
        render_state_sql_literal(value.py(), value, self.declared(column), None)
    }
}

/// The adapter's qualified name of the state table, failing without a schema.
fn qualified_table<'py>(
    render_qualified_name: &Bound<'py, PyAny>,
    (database, schema): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Option<String>> {
    let py: Python<'py> = render_qualified_name.py();
    let table_name: Bound<'py, PyAny> = PyString::new(py, MICROBATCH_TABLE_NAME).into_any();
    optional_text(&call_with(
        render_qualified_name,
        &[
            ("database", database.clone()),
            ("schema", schema.clone()),
            ("name", table_name),
        ],
    )?)
}

/// The state table of one scope: its target database and required schema.
fn scope_table(
    render_qualified_name: &Bound<'_, PyAny>,
    scope: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py: Python<'_> = scope.py();
    let schema: Bound<'_, PyAny> = native_field(scope, "target_schema")?;
    if schema.is_none() {
        return Err(state_error(py, MISSING_SCHEMA));
    }
    qualified_table(
        render_qualified_name,
        (&native_field(scope, "target_database")?, &schema),
    )?
    .ok_or_else(|| state_error(py, MISSING_SCHEMA))
}

/// The events as a list, and the table of the first event's scope; empty fails with `empty`.
fn events_table<'py>(
    events: &Bound<'py, PyAny>,
    render_qualified_name: &Bound<'py, PyAny>,
    empty: &str,
) -> PyResult<(Vec<Bound<'py, PyAny>>, String)> {
    let listed: Vec<Bound<'py, PyAny>> = events.try_iter()?.collect::<PyResult<_>>()?;
    let Some(first) = listed.first() else {
        return Err(state_error(events.py(), empty));
    };
    let table: String = scope_table(render_qualified_name, &native_field(first, "scope")?)?;
    Ok((listed, table))
}

/// Build one guarded set-based insert for events in the same state table.
#[pyfunction(name = "build_microbatch_insert_many_sql")]
#[pyo3(signature = (*, events, render_qualified_name, timestamp_type = None))]
fn insert_many(
    events: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    timestamp_type: Option<&str>,
) -> PyResult<String> {
    let (listed, table) = events_table(events, render_qualified_name, EMPTY_INSERT)?;
    let layout: Layout = Layout::new(events.py())?;
    let rows: Vec<Vec<String>> = listed
        .iter()
        .map(|event| layout.literals(event, timestamp_type))
        .collect::<PyResult<_>>()?;
    Ok(insert_many_sql(&table, &layout.columns, &rows))
}

/// Build one lookup for event IDs already present before a bulk insert.
#[pyfunction(name = "build_microbatch_existing_event_ids_sql")]
#[pyo3(signature = (*, events, render_qualified_name))]
fn existing_ids(
    events: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let (listed, table) = events_table(events, render_qualified_name, EMPTY_LOOKUP)?;
    let layout: Layout = Layout::new(events.py())?;
    let literals: Vec<String> = listed
        .iter()
        .map(|event| layout.literal(EVENT_ID_COLUMN, &event.getattr(EVENT_ID_COLUMN)?))
        .collect::<PyResult<_>>()?;
    Ok(existing_event_ids_sql(&table, &literals))
}

/// Build the ordered read of one scope's events, every generation for the wildcard.
#[pyfunction(name = "build_microbatch_read_scope_sql")]
#[pyo3(signature = (*, scope, render_qualified_name))]
fn read_scope(
    scope: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py: Python<'_> = scope.py();
    let table: String = scope_table(render_qualified_name, scope)?;
    let layout: Layout = Layout::new(py)?;
    let wildcard: String =
        host::entry(py, ExecutorHost::MicrobatchGenerationWildcard)?.extract()?;
    let generation: Bound<'_, PyAny> = native_field(scope, "physical_generation_id")?;
    let generation: Option<String> = if generation.extract::<String>()? == wildcard {
        None
    } else {
        Some(layout.literal("physical_generation_id", &generation)?)
    };
    let scope_kind: String = layout.literal("scope_kind", &native_field(scope, "scope_kind")?)?;
    let scope_key: String = layout.literal("scope_key", &native_field(scope, "scope_key")?)?;
    Ok(read_scope_sql(
        &table,
        &ScopeRead {
            columns: &layout.columns,
            scope_kind: &scope_kind,
            scope_key: &scope_key,
            generation: generation.as_deref(),
        },
    ))
}

/// Build direct microbatch-state DDL with the adapter's text and timestamp types.
#[pyfunction(name = "build_microbatch_create_table_sql")]
#[pyo3(signature = (**request))]
fn create_table(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("build_microbatch_create_table_sql", request)?;
    let py: Python<'_> = request.py();
    let table: String = qualified_table(
        &request.get("render_qualified_name")?,
        (&request.get("database")?, &request.get("schema")?),
    )?
    .ok_or_else(|| state_error(py, MISSING_SCHEMA))?;
    let render_framework_type: Bound<'_, PyAny> = request.get("render_framework_type")?;
    let framework: Bound<'_, PyAny> = host::entry(py, AdapterHost::FrameworkType)?;
    let text_type: String = render_framework_type
        .call1((native_field(&framework, "STRING")?,))?
        .extract()?;
    let timestamp_type: String = render_framework_type
        .call1((native_field(&framework, "TIMESTAMP")?,))?
        .extract()?;
    let layout: Layout = Layout::new(py)?;
    let columns: Vec<TableColumn> = layout
        .columns
        .iter()
        .zip(&layout.declared)
        .map(|(name, declared)| TableColumn {
            name: name.clone(),
            rendered_type: match declared.as_str() {
                "timestamp" => timestamp_type.clone(),
                "integer" => INTEGER_COLUMN_TYPE.to_owned(),
                _ => text_type.clone(),
            },
            required: false,
        })
        .collect();
    Ok(create_table_sql(&table, &columns))
}

/// Build non-unique scope and provenance indexes for indexed warehouses.
#[pyfunction(name = "build_microbatch_create_index_sqls")]
#[pyo3(signature = (**request))]
fn create_indexes<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("build_microbatch_create_index_sqls", request)?;
    let py: Python<'_> = request.py();
    let schema: Bound<'_, PyAny> = request.get("schema")?;
    let rendered: Bound<'_, PyAny> = call_with(
        &request.get("render_qualified_name")?,
        &[
            ("database", request.get("database")?),
            ("schema", schema.clone()),
            ("name", PyString::new(py, MICROBATCH_TABLE_NAME).into_any()),
        ],
    )?;
    let table: String = if rendered.is_truthy()? {
        rendered.extract()?
    } else {
        format!("{}.{MICROBATCH_TABLE_NAME}", schema.str()?)
    };
    let render_identifier: Bound<'_, PyAny> = request.get("render_identifier")?;
    let identifier = |name: &Bound<'_, PyAny>| -> PyResult<String> {
        render_identifier.call1((name,))?.extract()
    };
    let mut indexes: Vec<(String, Vec<String>)> = Vec::new();
    let definitions: Bound<'_, PyAny> = host::entry(py, ExecutorHost::MicrobatchDirectIndexes)?;
    for item in mapping_items(&definitions)?.try_iter()? {
        let (name, columns): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
        let columns: Vec<String> = columns
            .try_iter()?
            .map(|column| identifier(&column?))
            .collect::<PyResult<_>>()?;
        indexes.push((identifier(&name)?, columns));
    }
    PyTuple::new(py, create_index_sqls(&table, &indexes))
}

/// Return the content-addressed ID for one logical event.
#[pyfunction(name = "deterministic_microbatch_event_id")]
#[pyo3(signature = (**request))]
fn deterministic_event_id(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("deterministic_microbatch_event_id", request)?;
    let scope: Bound<'_, PyAny> = request.get("scope")?;
    let field = |name: &str| -> PyResult<String> { native_field(&scope, name)?.extract() };
    let optional =
        |name: &str| -> PyResult<Option<String>> { optional_text(&native_field(&scope, name)?) };
    let identity = MicrobatchEventIdentity {
        scope_kind: field("scope_kind")?,
        scope_key: field("scope_key")?,
        model_name: field("model_name")?,
        target_database: optional("target_database")?,
        target_schema: optional("target_schema")?,
        target_name: field("target_name")?,
        physical_generation_id: field("physical_generation_id")?,
        record_type: native_field(&request.get("record_type")?, "value")?.extract()?,
        partition_start: optional_text(&request.get("partition_start")?)?,
        partition_end: optional_text(&request.get("partition_end")?)?,
        completion_reason: request.get("completion_reason")?.extract()?,
    };
    microbatch_event_id(&identity).map_err(emit_error)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(insert_many, module)?)?;
    module.add_function(wrap_pyfunction!(existing_ids, module)?)?;
    module.add_function(wrap_pyfunction!(read_scope, module)?)?;
    module.add_function(wrap_pyfunction!(create_table, module)?)?;
    module.add_function(wrap_pyfunction!(create_indexes, module)?)?;
    module.add_function(wrap_pyfunction!(deterministic_event_id, module)?)
}
