//! A plan preview's adapter: warehouse reads come from the location's mirrored copy, nothing connects.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, PoisonError};

use chrono::SecondsFormat;
use pyo3::IntoPyObject;
use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python, pyclass, pymethods};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use serde::Deserialize;
use sqlbuild_analysis::fingerprint_storage::constants::FINGERPRINT_COLUMNS;
use sqlbuild_analysis::fingerprint_storage::main::read_latest_sql::read_latest_sql;
use sqlbuild_cache::digest::main::base64_text::base64_text;
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_core::constants::{FINGERPRINT_TABLE_NAME, STATE_TABLE_NAMES};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_orchestration::warehouse_metadata::models::{PreviewCopy, WarehouseCatalogRecord};
use sqlbuild_state::consumption::models::{
    ConsumedInput, ConsumptionAppend, ConsumptionRow, NodeKey, RelationKey,
};
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use sqlbuild_state::observations::constants::{CURSOR_MAX_KIND, SOURCE_COLUMNS_KIND};

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::native_host::consumption_state::Location;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::sqlite_connection::{SqliteConnection, SqliteCursor};
use crate::bindings::_helpers::records::fields::field as native_field;

create_exception!(
    sqlbuild._native,
    WarehouseAccessRefusedError,
    PyRuntimeError,
    "Raised when a plan preview reaches past the synced copy for the warehouse."
);

/// Adapter methods that only describe the dialect, never reaching the warehouse.
const PASSTHROUGH_NAMES: [&str; 12] = [
    "_render_source_relation_cast_subquery_with_columns",
    "adapter_name",
    "default_database",
    "default_schema",
    "default_table_promotion_mode",
    "expression_inference_profile",
    "metadata_inspection_concurrency",
    "python_functions_inherit_default_namespace",
    "requires_derived_table_aliases",
    "sql_analysis_dialect",
    "sql_analysis_dialect_name",
    "star_exclude_keyword",
];
/// Rendering and capability methods pass through; every other unknown attribute is refused.
const PASSTHROUGH_PREFIXES: [&str; 2] = ["render_", "supports_"];
/// Local table holding the copy's fingerprints; previews read it through per-location views.
const FINGERPRINT_STORE: &str = "__sqlbuild_synced_fingerprints";
const FINGERPRINT_STORE_COLUMNS: [&str; 2] = ["state_database", "state_schema"];
const MODEL_NODE_TYPE: &str = "model";

/// Items a preview reports as known only once the run plans live.
pub(crate) const PREVIEW_UNKNOWNS: [&str; 4] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
];

/// A source's described columns, keyed to the exact SQL that was described.
#[derive(Deserialize)]
struct DescribedColumns {
    sql_hash: String,
    columns: Vec<String>,
}

fn refused(access: &str) -> pyo3::PyErr {
    WarehouseAccessRefusedError::new_err(format!(
        "plan previews read only the synced copy; refused warehouse access: {access}"
    ))
}

fn same(left: Option<&str>, right: Option<&str>) -> bool {
    left.unwrap_or_default().to_lowercase() == right.unwrap_or_default().to_lowercase()
}

/// Whether `wanted` is unset or holds `value`, compared as the warehouse folds case.
fn any_same(wanted: Option<&[String]>, value: &str) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    for item in wanted {
        if same(Some(value), Some(item)) {
            return true;
        }
    }
    false
}

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// A connection that refuses every use; like the adapter, it lives in the adapter contract's module.
#[pyclass(
    frozen,
    module = "sqlbuild.adapter.contract",
    name = "SyncedCopyConnection"
)]
pub(crate) struct SyncedCopyConnection;

#[pymethods]
impl SyncedCopyConnection {
    fn __getattr__(&self, name: &str) -> PyResult<()> {
        Err(refused(&format!("connection.{name}")))
    }
}

/// Answers listing, column and fingerprint reads from the copy; dialect methods use `adapter`.
#[pyclass(
    frozen,
    module = "sqlbuild.adapter.contract",
    name = "SyncedCopyAdapter"
)]
pub(crate) struct SyncedCopyAdapter {
    adapter: Py<PyAny>,
    copy: PreviewCopy,
    store: SqliteConnection,
    views: Mutex<Vec<(Option<String>, String)>>,
    unknowns: Mutex<BTreeSet<String>>,
    windows: Mutex<BTreeMap<String, String>>,
}

impl SyncedCopyAdapter {
    /// The facade over `adapter` answering from the copy written at `path`.
    pub(crate) fn load(py: Python<'_>, adapter: Bound<'_, PyAny>, path: &str) -> PyResult<Self> {
        let text: String = std::fs::read_to_string(path)?;
        let copy: PreviewCopy = serde_json::from_str(&text)
            .map_err(|error| PyRuntimeError::new_err(format!("invalid synced copy: {error}")))?;
        let store: SqliteConnection = SqliteConnection::open(py, MEMORY_PATH)?;
        let columns: Vec<&str> = FINGERPRINT_STORE_COLUMNS
            .iter()
            .chain(FINGERPRINT_COLUMNS.iter())
            .copied()
            .collect();
        store.execute_native(
            py,
            &format!("CREATE TABLE {FINGERPRINT_STORE} ({})", columns.join(", ")),
            &[],
        )?;
        let placeholders: String = vec!["?"; columns.len()].join(", ");
        for record in &copy.fingerprints {
            let values: [Option<String>; 14] = [
                record.state_database.clone(),
                Some(record.state_schema.clone()),
                Some(record.node_type.clone()),
                Some(record.node_name.clone()),
                record.target_database.clone(),
                record.target_schema.clone(),
                record.target_name.clone(),
                Some(record.run_id.clone()),
                Some(record.definition_hash.clone()),
                Some(record.version_hash.clone()),
                Some(record.schema_fingerprint.clone()),
                Some(base64_text(record.definition.as_bytes())),
                Some(base64_text(record.metadata_json.as_bytes())),
                Some(record.ts.to_rfc3339_opts(SecondsFormat::AutoSi, true)),
            ];
            let values: Vec<rusqlite::types::Value> = values
                .into_iter()
                .map(|value| {
                    value.map_or(rusqlite::types::Value::Null, rusqlite::types::Value::Text)
                })
                .collect();
            store.execute_native(
                py,
                &format!("INSERT INTO {FINGERPRINT_STORE} VALUES ({placeholders})"),
                &values,
            )?;
        }
        Ok(Self {
            adapter: adapter.unbind(),
            copy,
            store,
            views: Mutex::new(Vec::new()),
            unknowns: Mutex::new(BTreeSet::new()),
            windows: Mutex::new(BTreeMap::new()),
        })
    }

    /// When the copy was synced, as ISO 8601 text; `None` before every required table synced.
    pub(crate) fn synced_at(&self) -> Option<String> {
        self.copy
            .synced_at
            .map(|at| at.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }

    /// What the preview could not know, in a stable order.
    pub(crate) fn unknowns(&self) -> Vec<String> {
        let mut unknowns: Vec<String> = PREVIEW_UNKNOWNS
            .iter()
            .map(|item| (*item).to_owned())
            .collect();
        unknowns.extend(
            self.unknowns
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .cloned(),
        );
        unknowns
    }

    /// Note one item the preview reports as unknown until the run plans live.
    pub(crate) fn mark_unknown(&self, item: String) {
        self.unknowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item);
    }

    /// Note that `model` reads from `start` up to the latest data when the run starts.
    pub(crate) fn mark_window(&self, model: &str, start: &str) {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(model.to_owned(), start.to_owned());
    }

    /// Each model's window start, read up to the latest data when the run starts.
    pub(crate) fn windows(&self) -> BTreeMap<String, String> {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The model's cursor `MAX` text, only when observed by its newest successful build.
    pub(crate) fn target_max(&self, model: &str) -> Option<&str> {
        for run in &self.copy.latest_runs {
            if run.node_type != MODEL_NODE_TYPE || run.node_name != model {
                continue;
            }
            for observation in &self.copy.observations {
                if observation.kind == CURSOR_MAX_KIND
                    && observation.node == model
                    && observation.run_id == run.run_id
                    && observation.state_schema == run.state_schema
                    && observation.state_database == run.state_database
                {
                    return Some(observation.value.as_str());
                }
            }
        }
        None
    }

    /// The copied consumption rows of the table in `location`, each relation's newest executions.
    pub(crate) fn consumption(&self, location: &Location) -> Vec<ConsumptionRow> {
        let mut rows: Vec<ConsumptionRow> = Vec::new();
        for record in &self.copy.consumption {
            if !same(Some(&record.state_schema), Some(&location.schema))
                || !(location.database.is_none()
                    || record.state_database.is_none()
                    || same(
                        record.state_database.as_deref(),
                        location.database.as_deref(),
                    ))
            {
                continue;
            }
            let text = |value: &Option<String>| value.clone().unwrap_or_default();
            rows.push(ConsumptionRow {
                row: ConsumptionAppend {
                    execution_id: text(&record.execution_id),
                    phase: text(&record.phase),
                    run_id: text(&record.run_id),
                    node: NodeKey {
                        kind: text(&record.node_type),
                        name: text(&record.node_name),
                    },
                    relation: RelationKey {
                        database: record.target_database.clone(),
                        schema: text(&record.target_schema),
                        name: text(&record.target_name),
                    },
                    version_hash: text(&record.version_hash),
                    input_count: record.input_count.unwrap_or(-1),
                    input: record.input_kind.clone().map(|kind| ConsumedInput {
                        kind,
                        name: text(&record.input_name),
                        version: text(&record.input_version),
                    }),
                },
                recorded_at: record.recorded_at.timestamp_micros(),
            });
        }
        rows
    }

    /// The source's columns as last described, only when it described exactly `sql`.
    pub(crate) fn described_columns(
        &self,
        source: &str,
        sql: &str,
    ) -> PyResult<Option<Vec<String>>> {
        let hash: String = sha256_hex(sql.as_bytes());
        for observation in &self.copy.observations {
            if observation.kind != SOURCE_COLUMNS_KIND || observation.node != source {
                continue;
            }
            let described: DescribedColumns =
                serde_json::from_str(&observation.value).map_err(|error| {
                    PyRuntimeError::new_err(format!(
                        "invalid source_columns observation for {source}: {error}"
                    ))
                })?;
            if described.sql_hash == hash {
                return Ok(Some(described.columns));
            }
        }
        Ok(None)
    }

    /// The relations a listing call named by its `database`, `schemas` and optional `names`.
    fn listed_by<'py>(
        &self,
        py: Python<'py>,
        function: &str,
        options: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Vec<&WarehouseCatalogRecord>> {
        let arguments: KeywordArguments<'_> = KeywordArguments::read(
            py,
            function,
            options,
            (&["connection", "database", "schemas"], &["names"]),
        )?;
        let database: Option<String> = arguments.value("database")?.extract()?;
        let schemas: Option<Vec<String>> = arguments.value("schemas")?.extract()?;
        let names: Option<Vec<String>> = arguments.value("names")?.extract()?;
        Ok(self.listed(database.as_deref(), schemas.as_deref(), names.as_deref()))
    }

    fn listed(
        &self,
        database: Option<&str>,
        schemas: Option<&[String]>,
        names: Option<&[String]>,
    ) -> Vec<&WarehouseCatalogRecord> {
        let mut listed: Vec<&WarehouseCatalogRecord> = Vec::new();
        for record in &self.copy.relations {
            if (STATE_TABLE_NAMES.contains(&record.name.as_str())
                && record.name != FINGERPRINT_TABLE_NAME)
                || !(database.is_none()
                    || record.state_database.is_none()
                    || same(record.state_database.as_deref(), database))
                || !any_same(schemas, &record.state_schema)
                || !any_same(names, &record.name)
            {
                continue;
            }
            listed.push(record);
        }
        listed
    }

    fn relation_info<'py>(
        &self,
        py: Python<'py>,
        record: &WarehouseCatalogRecord,
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &host::entry(py, AdapterHost::RelationInfo)?,
            &[
                (
                    "database",
                    record
                        .state_database
                        .as_deref()
                        .into_pyobject(py)?
                        .into_any(),
                ),
                ("schema", PyString::new(py, &record.state_schema).into_any()),
                ("name", PyString::new(py, &record.name).into_any()),
                (
                    "relation_type",
                    PyString::new(py, &record.relation_type).into_any(),
                ),
            ],
        )
    }

    fn columns<'py>(
        &self,
        py: Python<'py>,
        record: &WarehouseCatalogRecord,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let column_info: Bound<'py, PyAny> = host::entry(py, AdapterHost::ColumnInfo)?;
        let mut columns: Vec<Bound<'py, PyAny>> = Vec::with_capacity(record.columns.len());
        for column in &record.columns {
            columns.push(call_with(
                &column_info,
                &[
                    ("name", PyString::new(py, &column.name).into_any()),
                    ("type", PyString::new(py, &column.data_type).into_any()),
                ],
            )?);
        }
        PyTuple::new(py, columns)
    }
}

#[pymethods]
impl SyncedCopyAdapter {
    #[pyo3(signature = (connection_config = None))]
    fn connect(&self, connection_config: Option<Bound<'_, PyAny>>) -> SyncedCopyConnection {
        drop(connection_config);
        SyncedCopyConnection
    }

    #[pyo3(signature = (connection = None))]
    fn close(&self, connection: Option<Bound<'_, PyAny>>) {
        drop(connection);
    }

    #[pyo3(name = "list_relations", signature = (**options))]
    fn relation_listing<'py>(
        &self,
        py: Python<'py>,
        options: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let relations: Vec<Bound<'py, PyAny>> = self
            .listed_by(py, "list_relations", options)?
            .into_iter()
            .map(|record| self.relation_info(py, record))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, relations)
    }

    #[pyo3(name = "get_all_columns", signature = (**options))]
    fn all_columns<'py>(
        &self,
        py: Python<'py>,
        options: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let columns: Bound<'py, PyDict> = PyDict::new(py);
        for record in self.listed_by(py, "get_all_columns", options)? {
            columns.set_item(&record.name, self.columns(py, record)?)?;
        }
        Ok(columns)
    }

    #[pyo3(name = "get_columns_for_relations", signature = (*, connection, relations))]
    fn columns_by_relation<'py>(
        &self,
        py: Python<'py>,
        connection: Bound<'py, PyAny>,
        relations: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        drop(connection);
        let found: Bound<'py, PyDict> = PyDict::new(py);
        for relation in relations.try_iter()? {
            let relation: Bound<'py, PyAny> = relation?;
            let database: Option<String> = native_field(&relation, "database")?.extract()?;
            let schema: Option<String> = native_field(&relation, "schema")?.extract()?;
            let name: String = native_field(&relation, "name")?.extract()?;
            let schemas: Option<Vec<String>> = schema.clone().map(|schema| vec![schema]);
            let names: [String; 1] = [name.clone()];
            if let Some(record) = self
                .listed(database.as_deref(), schemas.as_deref(), Some(&names))
                .first()
            {
                found.set_item((database, schema, name), self.columns(py, record)?)?;
            }
        }
        Ok(found)
    }

    /// The latest-row read over this location's copied fingerprints, through a local view.
    #[pyo3(signature = (*, database, schema))]
    fn render_read_latest_fingerprints_sql(
        &self,
        py: Python<'_>,
        database: Option<String>,
        schema: String,
    ) -> PyResult<String> {
        let mut views = self.views.lock().unwrap_or_else(PoisonError::into_inner);
        let key: (Option<String>, String) = (database.clone(), schema.clone());
        let position: usize = match views.iter().position(|view| *view == key) {
            Some(position) => position,
            None => {
                let mut condition: String =
                    format!("lower(state_schema) = lower({})", literal(&schema));
                if let Some(database) = &database {
                    condition.push_str(&format!(
                        " AND (state_database IS NULL OR lower(state_database) = lower({}))",
                        literal(database)
                    ));
                }
                self.store.execute_native(
                    py,
                    &format!(
                        "CREATE VIEW {FINGERPRINT_STORE}_{} AS SELECT {} FROM {FINGERPRINT_STORE} \
                         WHERE {condition}",
                        views.len(),
                        FINGERPRINT_COLUMNS.join(", ")
                    ),
                    &[],
                )?;
                views.push(key);
                views.len() - 1
            }
        };
        Ok(read_latest_sql(&format!("{FINGERPRINT_STORE}_{position}")))
    }

    /// Only reads of the copied fingerprints run; any other statement would reach the warehouse.
    #[pyo3(signature = (*, connection, sql))]
    fn execute(
        &self,
        py: Python<'_>,
        connection: Bound<'_, PyAny>,
        sql: &str,
    ) -> PyResult<SqliteCursor> {
        drop(connection);
        if !sql.contains(&format!("{FINGERPRINT_STORE}_")) {
            return Err(refused(sql.trim().lines().next().unwrap_or_default()));
        }
        self.store.execute_native(py, sql, &[])
    }

    #[pyo3(signature = (*, connection, sql))]
    fn query_column_names(&self, connection: Bound<'_, PyAny>, sql: &str) -> PyResult<()> {
        drop(connection);
        Err(refused(&format!(
            "DESCRIBE {}",
            sql.trim().lines().next().unwrap_or_default()
        )))
    }

    /// Values and dialect methods come from the real adapter; any other method is refused.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let value: Bound<'py, PyAny> = native_field(self.adapter.bind(py), name)?;
        if !value.is_callable()
            || PASSTHROUGH_NAMES.contains(&name)
            || PASSTHROUGH_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            return Ok(value);
        }
        Err(refused(&format!("adapter.{name}")))
    }
}
