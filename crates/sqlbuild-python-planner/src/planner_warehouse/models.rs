//! Public records of planner warehouse.

use crate::planner_changes::main::without_declared_columns_hash::without_declared_columns_hash;
use crate::planner_migrations::main::fingerprint::fingerprint;
use crate::planner_migrations::main::migration_fingerprint_ref_names::migration_fingerprint_ref_names;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::CACHE_DATABASE_NAME;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::CACHE_DIRECTORY_NAME;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::CREATE_CACHE_TABLE_SQL;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::INSERT_SQL;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::MemoKey;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::SELECT_SQL;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::close;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::connect;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::count_computation;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::package_version;
use crate::planner_warehouse::_helpers::inspection::fingerprint_cache::sqlite_error;
use crate::planner_warehouse::_helpers::synced::synced_reads;
use crate::planner_warehouse::_helpers::synced::synced_reads::refused;
use crate::planner_warehouse::_helpers::synced::synced_reads::relation_columns;
use crate::planner_warehouse::_helpers::synced::synced_reads::relation_info;
use crate::planner_warehouse::constants::CURSOR_INPUTS;
use crate::planner_warehouse::constants::PREVIEW_MESSAGE;
use crate::planner_warehouse::constants::PREVIEW_UNKNOWNS;
use crate::planner_warehouse::main::reads::_unknown::unknown;
use crate::planner_warehouse::types::CursorProgress;
use crate::planner_warehouse::types::WarehouseReads;
use chrono::SecondsFormat;
use pyo3::exceptions::PyOSError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyBool;
use pyo3::types::PyDict;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyTuple;
use pyo3::types::{PyDictMethods, PyString};
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_entry::fingerprint_cache_entry;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_key::fingerprint_cache_key;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_lookup::fingerprint_cache_lookup;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_shared_digest::fingerprint_cache_shared_digest;
use sqlbuild_analysis::planner_warehouse::models::FingerprintCacheKey;
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::native_host::models::Location;
use sqlbuild_python_front::native_host::types::ConsumptionReads;
use sqlbuild_python_front::state_rows::models::EventTableLayout;
use sqlbuild_python_project::fingerprints::main::selected_set::selected_set;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_state::consumption::constants::RECENT_EXECUTIONS;
use sqlbuild_state::consumption::main::recent_executions::recent_executions;
use sqlbuild_state::consumption::models::ConsumptionRow;
use sqlbuild_state::warehouse_state::models::{
    InvalidObservation, RelationFilter, StateLocation, StateRelation, WarehouseState,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, PoisonError};

/// Reuse migration fingerprints across rename maps, planning phases and invocations.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct MigrationFingerprintCache {
    ref_names: HashMap<(String, String), Vec<String>>,
    fingerprints: HashMap<MemoKey, Option<String>>,
    database_path: Option<Py<PyAny>>,
    store: Option<Py<PyAny>>,
    store_opened: bool,
    pending: Vec<(String, String)>,
    shared_digest: String,
}

impl MigrationFingerprintCache {
    /// A cache persisting under `root`, or an in-memory cache without one.
    pub(crate) fn open(py: Python<'_>, root: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let root = root.filter(|root| !root.is_none());
        let database_path = match &root {
            Some(root) => Some(
                root.call_method1("joinpath", (CACHE_DIRECTORY_NAME, CACHE_DATABASE_NAME))?
                    .unbind(),
            ),
            None => None,
        };
        let shared_digest = if root.is_some() {
            fingerprint_cache_shared_digest(
                &package_version(py, "sqlbuild")?,
                &package_version(py, "polyglot-sql-chio")?,
            )
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?
        } else {
            String::new()
        };
        Ok(Self {
            ref_names: HashMap::new(),
            fingerprints: HashMap::new(),
            database_path,
            store: None,
            store_opened: false,
            pending: Vec::new(),
            shared_digest,
        })
    }

    fn cache_key(&self, key: &MemoKey) -> PyResult<String> {
        if self.database_path.is_none() {
            return Ok(String::new());
        }
        fingerprint_cache_key(&FingerprintCacheKey {
            shared_digest: self.shared_digest.clone(),
            query_sql: key.query_sql.clone(),
            metadata_json: key.metadata_json.clone(),
            dialect: key.dialect.clone(),
            visible_renames: key.visible_renames.clone(),
        })
        .map_err(|error| PyValueError::new_err(format!("{error:?}")))
    }

    fn open_store(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if self.store_opened {
            return Ok(self.store.as_ref().map(|store| store.clone_ref(py)));
        }
        self.store_opened = true;
        let Some(path) = &self.database_path else {
            return Ok(None);
        };
        let path = path.bind(py);
        if !path.call_method0("is_file")?.is_truthy()? {
            return Ok(None);
        }
        self.store = match connect(path) {
            Ok(store) => Some(store.unbind()),
            Err(error) if error.is_instance_of::<PyOSError>(py) || sqlite_error(py, &error)? => {
                None
            }
            Err(error) => return Err(error),
        };
        Ok(self.store.as_ref().map(|store| store.clone_ref(py)))
    }

    fn stored(&mut self, py: Python<'_>, cache_key: &str) -> PyResult<Option<Option<String>>> {
        let Some(store) = self.open_store(py)? else {
            return Ok(None);
        };
        let row = store
            .bind(py)
            .call_method1("execute", (SELECT_SQL, PyTuple::new(py, [cache_key])?))
            .and_then(|cursor| cursor.call_method0("fetchone"));
        let row = match row {
            Ok(row) => row,
            Err(error) if sqlite_error(py, &error)? => {
                self.store = None;
                close(py, Some(store))?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if row.is_none() {
            return Ok(None);
        }
        let contents = row.get_item(0)?;
        let Ok(contents) = contents.extract::<String>() else {
            return Ok(None);
        };
        Ok(fingerprint_cache_lookup(&contents, cache_key))
    }

    fn remember(&mut self, cache_key: &str, fingerprint: Option<&str>) -> PyResult<()> {
        if self.database_path.is_none() {
            return Ok(());
        }
        let entry = fingerprint_cache_entry(cache_key, fingerprint)
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
        match self.pending.iter_mut().find(|(key, _)| key == cache_key) {
            Some(existing) => existing.1 = entry,
            None => self.pending.push((cache_key.to_owned(), entry)),
        }
        Ok(())
    }

    fn write_pending(
        &self,
        py: Python<'_>,
        pending: &[(String, String)],
        store: Option<Py<PyAny>>,
    ) -> (Option<Py<PyAny>>, PyResult<()>) {
        let Some(path) = &self.database_path else {
            return (store, Ok(()));
        };
        if pending.is_empty() {
            return (store, Ok(()));
        }
        let path = path.bind(py);
        let store = match store {
            Some(store) => store,
            None => {
                let created = call_with(
                    &match native_field(path, "parent")
                        .and_then(|parent| native_field(&parent, "mkdir"))
                    {
                        Ok(mkdir) => mkdir,
                        Err(error) => return (None, Err(error)),
                    },
                    &[
                        ("parents", PyBool::new(py, true).to_owned().into_any()),
                        ("exist_ok", PyBool::new(py, true).to_owned().into_any()),
                    ],
                )
                .and_then(|_| connect(path));
                match created {
                    Ok(created) => created.unbind(),
                    Err(error) => return (None, Err(error)),
                }
            }
        };
        let written = (|| -> PyResult<()> {
            let open = store.bind(py);
            open.call_method1("execute", (CREATE_CACHE_TABLE_SQL,))?;
            let rows = PyList::empty(py);
            for (key, entry) in pending {
                rows.append(PyTuple::new(py, [key.as_str(), entry.as_str()])?)?;
            }
            open.call_method1("executemany", (INSERT_SQL, rows))?;
            open.call_method0("commit")?;
            Ok(())
        })();
        (Some(store), written)
    }
}

#[pymethods]
impl MigrationFingerprintCache {
    #[new]
    #[pyo3(signature = (*, root=None))]
    fn new(py: Python<'_>, root: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::open(py, root)
    }

    /// The migration fingerprint, keyed only by the renames this model can see.
    #[pyo3(signature = (**options))]
    fn fingerprint(
        &mut self,
        py: Python<'_>,
        options: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Option<String>> {
        let args = KeywordArguments::read(
            py,
            "fingerprint",
            options,
            (
                &["query_sql", "metadata_json", "ref_identities", "dialect"],
                &[],
            ),
        )?;
        let query_sql: String = args.value("query_sql")?.extract()?;
        let metadata_json: String = args.value("metadata_json")?.extract()?;
        let ref_identities = &args.value("ref_identities")?;
        let dialect: Option<String> = args.value("dialect")?.extract()?;
        let metadata_json: String = without_declared_columns_hash(&metadata_json);
        let definition = (query_sql.clone(), metadata_json.clone());
        let names = match self.ref_names.get(&definition) {
            Some(names) => names.clone(),
            None => {
                let found = object(
                    py,
                    migration_fingerprint_ref_names(py, &query_sql, &metadata_json)?,
                )?;
                let mut names: Vec<String> = found
                    .try_iter()?
                    .map(|name| name?.extract::<String>())
                    .collect::<PyResult<_>>()?;
                names.sort();
                self.ref_names.insert(definition, names.clone());
                names
            }
        };
        let mut visible: Vec<_> = Vec::new();
        for name in names {
            if ref_identities.contains(&name)? {
                let identity: String = ref_identities.get_item(&name)?.extract()?;
                visible.push((name, identity));
            }
        }
        let key: MemoKey = MemoKey {
            query_sql,
            metadata_json,
            dialect,
            visible_renames: visible,
        };
        if let Some(found) = self.fingerprints.get(&key) {
            return Ok(found.clone());
        }
        let cache_key = self.cache_key(&key)?;
        let stored = match self.stored(py, &cache_key)? {
            Some(stored) => stored,
            None => {
                count_computation(py, &key)?;
                let identities: BTreeMap<String, String> =
                    key.visible_renames.iter().cloned().collect();
                let computed: Option<String> = fingerprint(
                    &key.query_sql,
                    &key.metadata_json,
                    &identities,
                    key.dialect.as_deref(),
                )?;
                self.remember(&cache_key, computed.as_deref())?;
                computed
            }
        };
        self.fingerprints.insert(key, stored.clone());
        Ok(stored)
    }

    /// Write fingerprints computed since the last persist; failures only skip reuse.
    fn persist(&mut self, py: Python<'_>) -> PyResult<()> {
        let pending = std::mem::take(&mut self.pending);
        let store = self.store.take();
        self.store_opened = false;
        let (store, written) = self.write_pending(py, &pending, store);
        let closed = close(py, store);
        match written {
            Err(error) if error.is_instance_of::<PyOSError>(py) || sqlite_error(py, &error)? => {}
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        closed
    }
}

/// Migration fingerprints computed since the last reset, for work-count tests.
#[pyclass]
#[derive(Debug, Default)]
pub(crate) struct FingerprintWork {
    pub computations: u64,
    pub distinct: BTreeSet<u64>,
}

/// Everything one snapshot gather needs.
#[derive(Debug)]
pub struct SnapshotRequest<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) dialect: Bound<'py, Dialect>,
    pub(crate) reads: Bound<'py, PlannerReads>,
    pub(crate) selected_keys: Bound<'py, PyAny>,
    pub(crate) full_refresh: bool,
    pub(crate) full_refresh_model_names: Bound<'py, PyAny>,
    pub(crate) on_progress: Bound<'py, PyAny>,
    pub(crate) deferred_locations: Bound<'py, PyAny>,
    pub(crate) cursor_scope: Bound<'py, PyAny>,
    /// Python nodes' `(database, schema)`; their fingerprints are read there too. `None` when unknown.
    pub(crate) node_results_location: Option<(Option<String>, Option<String>)>,
}

/// One warehouse read's answer: what the warehouse holds, or the item a plan reports as unknown.
pub enum Read<T> {
    Known(T),
    Unknown(Cow<'static, str>),
}

/// A read the warehouse adapter answers in one call.
pub enum AdapterRead<'py> {
    /// Each table freshness request's metadata, keyed by request.
    TableFreshness(Bound<'py, PyAny>),
    /// One table's `[database, schema, name]` freshness metadata.
    SingleTableFreshness([Bound<'py, PyAny>; 3]),
    /// The rows of one source freshness query.
    FreshnessQuery(Bound<'py, PyAny>),
    /// Why a destination cannot rename columns, `None` when it can.
    ColumnRenameUnavailable(Bound<'py, PyAny>),
    /// One dynamic model's `[database, schema, name]` listed target lag and refresh warehouse.
    DynamicRefresh([Bound<'py, PyAny>; 3]),
}

/// A read of what the warehouse state tables recorded.
pub enum StateHistoryRead<'a, 'py> {
    /// The latest recorded source freshness of `(database, schema)`, as a latest-records read.
    SourceFreshness {
        location: (&'a Bound<'py, PyAny>, &'a Bound<'py, PyAny>),
        table_exists: bool,
        source_names: Option<&'a [String]>,
    },
    /// The migration state tables of `schemas` in `database`, narrowed to `names`.
    MigrationTables {
        database: &'a Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    },
    /// The recorded migration events of one state table in `(database, schema)`, as a tuple.
    MigrationEvents {
        layout: &'a EventTableLayout,
        location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
        stored: Option<&'a [String]>,
    },
}

/// Where to read the newest fingerprints of one state location, narrowed as a filtered read is.
#[derive(Debug)]
pub struct FingerprintRead<'a> {
    pub(crate) database: Option<String>,
    pub(crate) schema: &'a str,
    pub(crate) table_exists: bool,
    /// Node names to keep, or every node when `None`.
    pub(crate) node_names: Option<Vec<String>>,
    /// Node types the name filter applies to; other types are kept whole.
    pub(crate) filtered_node_types: Vec<String>,
}

/// One target cursor `MAX` at or before the latest allowed start, for one model.
#[derive(Debug)]
pub struct EligibleCursorQuery {
    pub(crate) model_name: String,
    pub(crate) target: String,
    pub(crate) cursor_column: String,
    pub(crate) sql: String,
}

/// One `UNION ALL` statement answering several cursor reads, one row per read led by `_read`.
#[derive(Debug)]
pub struct CursorBatch {
    /// The reads it answers in `_read` order: each read's index and whether its row has a minimum.
    pub(crate) reads: Vec<(usize, bool)>,
    pub(crate) sql: String,
}

/// A plan's cursor reads: one statement per read, and batches answering the reads together.
#[derive(Debug)]
pub struct CursorStatements {
    pub(crate) singles: Vec<String>,
    /// Empty when the reads run one statement each.
    pub(crate) batches: Vec<CursorBatch>,
}

/// The warehouse reads one plan runs through.
#[pyclass(frozen, module = "sqlbuild._native", name = "WarehouseReads")]
#[derive(Debug)]
pub struct PlannerReads {
    reads: Arc<dyn WarehouseReads>,
}

impl PlannerReads {
    pub fn new(reads: Arc<dyn WarehouseReads>) -> Self {
        Self { reads }
    }

    /// The reads behind this handle.
    pub fn reads(&self) -> &dyn WarehouseReads {
        self.reads.as_ref()
    }

    /// The reads a keyword argument passed; anything else is refused.
    pub fn passed(value: Bound<'_, PyAny>) -> PyResult<Bound<'_, PlannerReads>> {
        Ok(value.cast_into::<PlannerReads>()?)
    }
}

impl ConsumptionReads for dyn WarehouseReads + '_ {
    fn read_recent(&self, py: Python<'_>, location: &Location) -> PyResult<Vec<ConsumptionRow>> {
        self.recent_consumption(py, location)
    }

    fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        self.consumption_executions(py, location, execution_ids)
    }
}

/// The reads of one plan preview over a synced state, and what the plan found unknown.
#[derive(Debug)]
pub struct SyncedReads {
    state: Arc<WarehouseState>,
    unknowns: Mutex<BTreeSet<String>>,
    windows: Mutex<BTreeMap<String, String>>,
}

impl SyncedReads {
    /// Reads over `state`, which several plans may share.
    pub fn new(state: Arc<WarehouseState>) -> Self {
        Self {
            state,
            unknowns: Mutex::new(BTreeSet::new()),
            windows: Mutex::new(BTreeMap::new()),
        }
    }

    /// When the state was synced, as ISO 8601 text; `None` before every required table synced.
    pub fn synced_at(&self) -> Option<String> {
        self.state
            .synced_at()
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
                .filter(|item| !PREVIEW_UNKNOWNS.contains(&item.as_str()))
                .cloned(),
        );
        unknowns
    }

    /// Each model's window start, read up to the latest data when the run starts.
    pub fn windows(&self) -> BTreeMap<String, String> {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Every synced consumption row of the table in `(database, schema)`.
    pub(crate) fn consumption(
        &self,
        (database, schema): (Option<&str>, &str),
    ) -> Vec<ConsumptionRow> {
        self.state.consumption_rows(&StateLocation {
            database: database.map(str::to_owned),
            schema: schema.to_owned(),
        })
    }

    fn listed<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        (schemas, names): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyTuple>> {
        let database: Option<String> = database.extract()?;
        let schemas: Option<Vec<String>> = schemas.extract()?;
        let names: Option<Vec<String>> = names.extract()?;
        let relations: Vec<Bound<'py, PyAny>> = self
            .state
            .relations(&RelationFilter {
                database: database.as_deref(),
                schemas: schemas.as_deref(),
                names: names.as_deref(),
            })
            .into_iter()
            .map(|relation| relation_info(py, relation))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, relations)
    }
}

impl WarehouseReads for SyncedReads {
    fn preview(&self) -> bool {
        true
    }

    fn relations<'py>(
        &self,
        py: Python<'py>,
        database: &Bound<'py, PyAny>,
        filter: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.listed(py, database, filter)?.into_any())
    }

    fn columns<'py>(
        &self,
        py: Python<'py>,
        relations: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let found: Bound<'py, PyDict> = PyDict::new(py);
        for relation in relations.try_iter()? {
            let relation: Bound<'py, PyAny> = relation?;
            let database: Option<String> = native_field(&relation, "database")?.extract()?;
            let schema: Option<String> = native_field(&relation, "schema")?.extract()?;
            let name: String = native_field(&relation, "name")?.extract()?;
            let schemas: Option<Vec<String>> = schema.clone().map(|schema| vec![schema]);
            let names: [String; 1] = [name.clone()];
            let listed: Vec<&StateRelation> = self.state.relations(&RelationFilter {
                database: database.as_deref(),
                schemas: schemas.as_deref(),
                names: Some(&names),
            });
            if let Some(listed) = listed.first() {
                found.set_item((database, schema, name), relation_columns(py, listed)?)?;
            }
        }
        Ok(found.into_any())
    }

    fn fingerprints<'py>(
        &self,
        py: Python<'py>,
        read: FingerprintRead<'_>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if !read.table_exists {
            return new_record_with(
                py,
                RecordId::FingerprintSet,
                &[
                    ("schema", PyString::new(py, read.schema).into_any()),
                    ("fingerprints", PyDict::new(py).into_any()),
                ],
            );
        }
        let datetime: Bound<'py, PyAny> = imported(py, "datetime", "datetime")?;
        let location = StateLocation {
            database: read.database,
            schema: read.schema.to_owned(),
        };
        let latest: Vec<Bound<'py, PyAny>> = self
            .state
            .latest_fingerprints(&location)
            .into_iter()
            .map(|stored| synced_reads::fingerprint(&datetime, stored))
            .collect::<PyResult<_>>()?;
        selected_set(
            py,
            &location.schema,
            latest,
            (read.node_names, read.filtered_node_types),
        )
    }

    fn within_inspection<'py>(
        &self,
        _py: Python<'py>,
        plan: &mut dyn FnMut() -> PyResult<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        plan()
    }

    fn inspection_concurrency(&self, _py: Python<'_>) -> PyResult<i64> {
        Ok(1)
    }

    fn prefetch_schema_listings<'py>(
        &self,
        _py: Python<'py>,
        _scopes: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        Ok(())
    }

    fn lists_by_schema(&self, _py: Python<'_>) -> PyResult<bool> {
        Ok(false)
    }

    fn cursor_bounds<'py>(
        &self,
        _py: Python<'py>,
        _statements: &dyn Fn() -> PyResult<CursorStatements>,
        _progress: &dyn CursorProgress<'py>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(Read::Unknown(CURSOR_INPUTS.into()))
    }

    fn eligible_cursor_maxes<'py>(
        &self,
        _py: Python<'py>,
        _queries: Vec<EligibleCursorQuery>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(Read::Unknown(CURSOR_INPUTS.into()))
    }

    fn target_max(&self, model: &str) -> Read<String> {
        match self.state.target_max(model) {
            Some(observed) => Read::Known(observed.to_owned()),
            None => Read::Unknown(format!("cursor position of {model}").into()),
        }
    }

    fn note_window(&self, model: &str, start: &str) {
        self.windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(model.to_owned(), start.to_owned());
    }

    fn note_unknown(&self, item: String) {
        self.unknowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(item);
    }

    fn described_columns(
        &self,
        _py: Python<'_>,
        source: &str,
        sql: &str,
    ) -> PyResult<Read<Vec<String>>> {
        let described = self.state.described_columns(source, sql).map_err(
            |InvalidObservation { source, reason }| {
                PyRuntimeError::new_err(format!(
                    "invalid source_columns observation for {source}: {reason}"
                ))
            },
        )?;
        Ok(match described {
            Some(described) => Read::Known(described),
            None => unknown(
                self,
                format!("query columns of expression source {source} (planned as declared)"),
            ),
        })
    }

    fn read_access<'py>(
        &self,
        _py: Python<'py>,
        _relations: &dyn Fn() -> PyResult<Vec<Bound<'py, PyAny>>>,
    ) -> PyResult<Read<Vec<Bound<'py, PyAny>>>> {
        Ok(unknown(self, "read access to these sources"))
    }

    fn state_history<'py>(
        &self,
        _py: Python<'py>,
        read: StateHistoryRead<'_, 'py>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(
            self,
            match read {
                StateHistoryRead::SourceFreshness { .. } => "source freshness",
                StateHistoryRead::MigrationTables { .. }
                | StateHistoryRead::MigrationEvents { .. } => "recorded migrations",
            },
        ))
    }

    fn adapter_read<'py>(
        &self,
        py: Python<'py>,
        read: AdapterRead<'py>,
    ) -> PyResult<Read<Bound<'py, PyAny>>> {
        let requests: Bound<'py, PyAny> = match read {
            AdapterRead::TableFreshness(requests) => requests,
            AdapterRead::SingleTableFreshness(_) | AdapterRead::FreshnessQuery(_) => {
                return Ok(unknown(self, "source freshness"));
            }
            AdapterRead::ColumnRenameUnavailable(_) => {
                return Ok(unknown(self, "column rename support"));
            }
            AdapterRead::DynamicRefresh(_) => {
                return Ok(unknown(self, "dynamic model listings"));
            }
        };
        let unavailable =
            host_entry(py, AdapterHost::TableFreshnessMetadata)?.getattr("unavailable")?;
        let outcome = call_with(
            &unavailable,
            &[("message", PyString::new(py, PREVIEW_MESSAGE).into_any())],
        )?;
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        for request in requests.try_iter()? {
            metadata.set_item(request?, &outcome)?;
        }
        Ok(Read::Known(metadata.into_any()))
    }

    fn table_types(&self) -> Read<()> {
        unknown(self, "table types")
    }

    fn retention<'py>(&self, _py: Python<'py>) -> PyResult<Read<Bound<'py, PyAny>>> {
        Ok(unknown(self, "retention"))
    }

    fn old_name_view_columns<'py>(
        &self,
        _py: Python<'py>,
        _location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Read<Vec<String>>> {
        Ok(unknown(self, "recorded migrations"))
    }

    fn deferred_relations<'py>(
        &self,
        _py: Python<'py>,
        _location: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    ) -> PyResult<Bound<'py, PyAny>> {
        Err(refused("deferred target relations"))
    }

    fn recent_consumption(
        &self,
        _py: Python<'_>,
        location: &Location,
    ) -> PyResult<Vec<ConsumptionRow>> {
        Ok(recent_executions(
            &self.consumption((location.database.as_deref(), &location.schema)),
            RECENT_EXECUTIONS,
        ))
    }

    fn consumption_executions(
        &self,
        _py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>> {
        let mut rows: Vec<ConsumptionRow> =
            self.consumption((location.database.as_deref(), &location.schema));
        rows.retain(|row| execution_ids.contains(&row.row.execution_id.as_str()));
        Ok(rows)
    }

    fn close_consumption(&self, _py: Python<'_>) -> PyResult<()> {
        Ok(())
    }
}
