//! Public records of warehouse access.

use crate::boundary::main::call_with::call_with;
use crate::boundary::main::imported::imported;
use crate::boundary::models::KeywordArguments;
use crate::native_host::main::host_entry::host_entry;
use crate::records::main::fields::field::field as native_field;
use crate::warehouse_access::_helpers::sqlite_connection::database_error;
use crate::warehouse_access::_helpers::synced_copy::DescribedColumns;
use crate::warehouse_access::_helpers::synced_copy::FINGERPRINT_STORE;
use crate::warehouse_access::_helpers::synced_copy::FINGERPRINT_STORE_COLUMNS;
use crate::warehouse_access::_helpers::synced_copy::MODEL_NODE_TYPE;
use crate::warehouse_access::_helpers::synced_copy::PASSTHROUGH_NAMES;
use crate::warehouse_access::_helpers::synced_copy::PASSTHROUGH_PREFIXES;
use crate::warehouse_access::_helpers::synced_copy::PREVIEW_UNKNOWNS;
use crate::warehouse_access::_helpers::synced_copy::SyncedCopyConnection;
use crate::warehouse_access::_helpers::synced_copy::any_same;
use crate::warehouse_access::_helpers::synced_copy::literal;
use crate::warehouse_access::_helpers::synced_copy::refused;
use crate::warehouse_access::_helpers::synced_copy::same;
use chrono::SecondsFormat;
use pyo3::IntoPyObject;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::pyclass;
use pyo3::prelude::pymethods;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::params_from_iter;
use rusqlite::types::Value;
use sqlbuild_analysis::fingerprint_storage::constants::FINGERPRINT_COLUMNS;
use sqlbuild_analysis::fingerprint_storage::main::read_latest_sql::read_latest_sql;
use sqlbuild_cache::digest::main::base64_text::base64_text;
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_core::constants::STATE_TABLE_NAMES;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_orchestration::warehouse_metadata::models::PreviewCopy;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseCatalogRecord;
use sqlbuild_state::consumption::models::ConsumedInput;
use sqlbuild_state::consumption::models::ConsumptionAppend;
use sqlbuild_state::consumption::models::ConsumptionRow;
use sqlbuild_state::consumption::models::NodeKey;
use sqlbuild_state::consumption::models::RelationKey;
use sqlbuild_state::execution_history::constants::MEMORY_PATH;
use sqlbuild_state::observations::constants::CURSOR_MAX_KIND;
use sqlbuild_state::observations::constants::SOURCE_COLUMNS_KIND;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteConnection")]
#[derive(Debug)]
pub struct SqliteConnection {
    pub(crate) connection: Mutex<Option<Connection>>,
}

impl SqliteConnection {
    /// Open literal filenames, using absolute paths because bundled SQLite enables URI handling globally.
    pub fn open(py: Python<'_>, path: &str) -> PyResult<Self> {
        let flags = OpenFlags::default() & !OpenFlags::SQLITE_OPEN_URI;
        let database: PathBuf = if path == MEMORY_PATH {
            PathBuf::from(path)
        } else {
            std::env::current_dir()?.join(path)
        };
        let connection = py
            .detach(|| Connection::open_with_flags(database, flags))
            .map_err(|error| database_error(py, error))?;
        Ok(Self {
            connection: Mutex::new(Some(connection)),
        })
    }

    pub(crate) fn with_connection<T: Send>(
        &self,
        py: Python<'_>,
        body: impl FnOnce(&Connection) -> rusqlite::Result<T> + Send,
    ) -> PyResult<T> {
        let result = py.detach(|| {
            let guard = self
                .connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard.as_ref().map(body)
        });
        match result {
            Some(result) => result.map_err(|error| database_error(py, error)),
            None => Err(pyo3::prelude::PyErr::from_value(
                imported(py, "sqlite3", "ProgrammingError")?
                    .call1(("Cannot operate on a closed database.",))?,
            )),
        }
    }

    pub fn execute_native(
        &self,
        py: Python<'_>,
        sql: &str,
        params: &[Value],
    ) -> PyResult<SqliteCursor> {
        self.with_connection(py, |connection| {
            let mut statement = connection.prepare_cached(sql)?;
            let columns = Arc::new(
                statement
                    .column_names()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            );
            let count: usize = statement.column_count();
            let mut cursor = statement.query(params_from_iter(params))?;
            let mut rows: VecDeque<SqliteRow> = VecDeque::new();
            while let Some(row) = cursor.next()? {
                rows.push_back(SqliteRow {
                    columns: Arc::clone(&columns),
                    values: (0..count)
                        .map(|index| row.get(index))
                        .collect::<rusqlite::Result<_>>()?,
                });
            }
            Ok(SqliteCursor {
                rows: Mutex::new(rows),
                last_row_id: connection.last_insert_rowid(),
            })
        })
    }
}

#[pymethods]
impl SqliteConnection {
    #[pyo3(signature = (sql, parameters = None))]
    pub(crate) fn execute(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<SqliteCursor> {
        let mut params: Vec<Value> = Vec::new();
        if let Some(parameters) = parameters {
            for value in parameters.try_iter()? {
                let value = value?;
                params.push(if value.is_none() {
                    Value::Null
                } else if let Ok(value) = value.extract::<i64>() {
                    Value::Integer(value)
                } else {
                    Value::Text(value.extract()?)
                });
            }
        }
        self.execute_native(py, sql, &params)
    }

    pub(crate) fn executemany(
        &self,
        py: Python<'_>,
        sql: &str,
        parameters: &Bound<'_, PyAny>,
    ) -> PyResult<SqliteCursor> {
        let mut result = SqliteCursor {
            rows: Mutex::default(),
            last_row_id: 0,
        };
        for row in parameters.try_iter()? {
            result = self.execute(py, sql, Some(&row?))?;
        }
        Ok(result)
    }

    #[getter]
    pub fn in_transaction(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_connection(py, |connection| Ok(!connection.is_autocommit()))
    }

    pub fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            self.connection
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
        });
        Ok(())
    }
}

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteRow")]
#[derive(Debug)]
pub struct SqliteRow {
    pub columns: Arc<Vec<String>>,
    pub values: Vec<Value>,
}

impl SqliteRow {
    pub fn value<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyAny>> {
        match self.values.get(index) {
            Some(Value::Null) => Ok(py.None().into_bound(py)),
            Some(Value::Integer(value)) => value.into_bound_py_any(py),
            Some(Value::Real(value)) => value.into_bound_py_any(py),
            Some(Value::Text(value)) => value.into_bound_py_any(py),
            Some(Value::Blob(value)) => Ok(PyBytes::new(py, value).into_any()),
            None => Err(PyIndexError::new_err("tuple index out of range")),
        }
    }

    pub fn column<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let index: usize = self
            .columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(name))
            .ok_or_else(|| PyIndexError::new_err("No item with that key"))?;
        self.value(py, index)
    }
}

#[pymethods]
impl SqliteRow {
    pub(crate) fn __getitem__<'py>(&self, key: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        if let Ok(index) = key.extract::<isize>() {
            let index = if index < 0 {
                self.values.len().checked_add_signed(index)
            } else {
                Some(index as usize)
            };
            return self.value(key.py(), index.unwrap_or(usize::MAX));
        }
        if let Ok(name) = key.extract::<String>() {
            return self.column(key.py(), &name);
        }
        Err(PyTypeError::new_err("Index must be int or string"))
    }
}

#[pyclass(frozen, module = "sqlbuild._native", name = "_HistorySqliteCursor")]
#[derive(Debug)]
pub struct SqliteCursor {
    pub rows: Mutex<VecDeque<SqliteRow>>,
    pub last_row_id: i64,
}

impl SqliteCursor {
    pub fn next_row(&self) -> Option<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
    }
}

#[pymethods]
impl SqliteCursor {
    pub(crate) fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    pub(crate) fn __next__(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    pub(crate) fn fetchone(&self) -> Option<SqliteRow> {
        self.next_row()
    }

    pub(crate) fn fetchall(&self) -> Vec<SqliteRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    #[getter]
    pub(crate) fn lastrowid(&self) -> i64 {
        self.last_row_id
    }
}

/// Answers listing, column and fingerprint reads from the copy; dialect methods use `adapter`.
#[pyclass(
    frozen,
    module = "sqlbuild.adapter.contract",
    name = "SyncedCopyAdapter"
)]
#[derive(Debug)]
pub struct SyncedCopyAdapter {
    pub(crate) adapter: Py<PyAny>,
    pub(crate) copy: PreviewCopy,
    pub(crate) store: SqliteConnection,
    pub(crate) views: Mutex<Vec<(Option<String>, String)>>,
    pub(crate) unknowns: Mutex<BTreeSet<String>>,
    pub(crate) windows: Mutex<BTreeMap<String, String>>,
}

impl SyncedCopyAdapter {
    /// The facade over `adapter` answering from the copy written at `path`.
    pub fn load(py: Python<'_>, adapter: Bound<'_, PyAny>, path: &str) -> PyResult<Self> {
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
    pub fn synced_at(&self) -> Option<String> {
        self.copy
            .synced_at
            .map(|at| at.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }

    /// What the preview could not know, in a stable order.
    pub fn unknowns(&self) -> Vec<String> {
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
    pub fn mark_unknown(&self, item: String) {
        self.unknowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item);
    }

    /// Note that `model` reads from `start` up to the latest data when the run starts.
    pub fn mark_window(&self, model: &str, start: &str) {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(model.to_owned(), start.to_owned());
    }

    /// Each model's window start, read up to the latest data when the run starts.
    pub fn windows(&self) -> BTreeMap<String, String> {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The model's cursor `MAX` text, only when observed by its newest successful build.
    pub fn target_max(&self, model: &str) -> Option<&str> {
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

    /// Every copied consumption row of the table in `(database, schema)`.
    pub fn consumption(&self, (database, schema): (Option<&str>, &str)) -> Vec<ConsumptionRow> {
        let mut rows: Vec<ConsumptionRow> = Vec::new();
        for record in &self.copy.consumption {
            if !same(Some(&record.state_schema), Some(schema))
                || !(database.is_none()
                    || record.state_database.is_none()
                    || same(record.state_database.as_deref(), database))
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
    pub fn described_columns(&self, source: &str, sql: &str) -> PyResult<Option<Vec<String>>> {
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
    pub(crate) fn listed_by<'py>(
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

    pub(crate) fn listed(
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

    pub(crate) fn relation_info<'py>(
        &self,
        py: Python<'py>,
        record: &WarehouseCatalogRecord,
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &host_entry(py, AdapterHost::RelationInfo)?,
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

    pub(crate) fn columns<'py>(
        &self,
        py: Python<'py>,
        record: &WarehouseCatalogRecord,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let column_info: Bound<'py, PyAny> = host_entry(py, AdapterHost::ColumnInfo)?;
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
    pub(crate) fn connect(
        &self,
        connection_config: Option<Bound<'_, PyAny>>,
    ) -> SyncedCopyConnection {
        drop(connection_config);
        SyncedCopyConnection
    }

    #[pyo3(signature = (connection = None))]
    pub(crate) fn close(&self, connection: Option<Bound<'_, PyAny>>) {
        drop(connection);
    }

    #[pyo3(name = "list_relations", signature = (**options))]
    pub(crate) fn relation_listing<'py>(
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
    pub(crate) fn all_columns<'py>(
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
    pub(crate) fn columns_by_relation<'py>(
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
    pub(crate) fn render_read_latest_fingerprints_sql(
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
    pub(crate) fn execute(
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
    pub(crate) fn query_column_names(
        &self,
        connection: Bound<'_, PyAny>,
        sql: &str,
    ) -> PyResult<()> {
        drop(connection);
        Err(refused(&format!(
            "DESCRIBE {}",
            sql.trim().lines().next().unwrap_or_default()
        )))
    }

    /// Values and dialect methods come from the real adapter; any other method is refused.
    pub(crate) fn __getattr__<'py>(
        &self,
        py: Python<'py>,
        name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
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
