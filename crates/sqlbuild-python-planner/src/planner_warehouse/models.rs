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
use pyo3::exceptions::PyOSError;
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
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_entry::fingerprint_cache_entry;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_key::fingerprint_cache_key;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_lookup::fingerprint_cache_lookup;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_shared_digest::fingerprint_cache_shared_digest;
use sqlbuild_analysis::planner_warehouse::models::FingerprintCacheKey;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::object::object;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;

/// Reuse migration fingerprints across rename maps, planning phases and invocations.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug)]
pub struct MigrationFingerprintCache {
    pub(crate) ref_names: HashMap<(String, String), Vec<String>>,
    pub(crate) fingerprints: HashMap<MemoKey, Option<String>>,
    pub(crate) database_path: Option<Py<PyAny>>,
    pub(crate) connection: Option<Py<PyAny>>,
    pub(crate) connection_opened: bool,
    pub(crate) pending: Vec<(String, String)>,
    pub(crate) shared_digest: String,
}

impl MigrationFingerprintCache {
    /// A cache persisting under `root`, or an in-memory cache without one.
    pub fn open(py: Python<'_>, root: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let root = root.filter(|root| !root.is_none());
        let database_path = match &root {
            Some(root) => Some(
                root.call_method1("__truediv__", (CACHE_DIRECTORY_NAME,))?
                    .call_method1("__truediv__", (CACHE_DATABASE_NAME,))?
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
            connection: None,
            connection_opened: false,
            pending: Vec::new(),
            shared_digest,
        })
    }

    pub(crate) fn cache_key(&self, key: &MemoKey) -> PyResult<String> {
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

    pub(crate) fn open_connection(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        if self.connection_opened {
            return Ok(self
                .connection
                .as_ref()
                .map(|connection| connection.clone_ref(py)));
        }
        self.connection_opened = true;
        let Some(path) = &self.database_path else {
            return Ok(None);
        };
        let path = path.bind(py);
        if !path.call_method0("is_file")?.is_truthy()? {
            return Ok(None);
        }
        self.connection = match connect(path) {
            Ok(connection) => Some(connection.unbind()),
            Err(error) if error.is_instance_of::<PyOSError>(py) || sqlite_error(py, &error)? => {
                None
            }
            Err(error) => return Err(error),
        };
        Ok(self
            .connection
            .as_ref()
            .map(|connection| connection.clone_ref(py)))
    }

    pub(crate) fn stored(
        &mut self,
        py: Python<'_>,
        cache_key: &str,
    ) -> PyResult<Option<Option<String>>> {
        let Some(connection) = self.open_connection(py)? else {
            return Ok(None);
        };
        let row = connection
            .bind(py)
            .call_method1("execute", (SELECT_SQL, PyTuple::new(py, [cache_key])?))
            .and_then(|cursor| cursor.call_method0("fetchone"));
        let row = match row {
            Ok(row) => row,
            Err(error) if sqlite_error(py, &error)? => {
                self.connection = None;
                close(py, Some(connection))?;
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

    pub(crate) fn remember(&mut self, cache_key: &str, fingerprint: Option<&str>) -> PyResult<()> {
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

    pub(crate) fn write_pending(
        &self,
        py: Python<'_>,
        pending: &[(String, String)],
        connection: Option<Py<PyAny>>,
    ) -> (Option<Py<PyAny>>, PyResult<()>) {
        let Some(path) = &self.database_path else {
            return (connection, Ok(()));
        };
        if pending.is_empty() {
            return (connection, Ok(()));
        }
        let path = path.bind(py);
        let connection = match connection {
            Some(connection) => connection,
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
            let open = connection.bind(py);
            open.call_method1("execute", (CREATE_CACHE_TABLE_SQL,))?;
            let rows = PyList::empty(py);
            for (key, entry) in pending {
                rows.append(PyTuple::new(py, [key.as_str(), entry.as_str()])?)?;
            }
            open.call_method1("executemany", (INSERT_SQL, rows))?;
            open.call_method0("commit")?;
            Ok(())
        })();
        (Some(connection), written)
    }
}

#[pymethods]
impl MigrationFingerprintCache {
    #[new]
    #[pyo3(signature = (*, root=None))]
    pub(crate) fn new(py: Python<'_>, root: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        Self::open(py, root)
    }

    /// The migration fingerprint, keyed only by the renames this model can see.
    #[pyo3(signature = (**options))]
    pub(crate) fn fingerprint(
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
    pub(crate) fn persist(&mut self, py: Python<'_>) -> PyResult<()> {
        let pending = std::mem::take(&mut self.pending);
        let connection = self.connection.take();
        self.connection_opened = false;
        let (connection, written) = self.write_pending(py, &pending, connection);
        let closed = close(py, connection);
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
    pub project: Bound<'py, PyAny>,
    pub adapter: Bound<'py, PyAny>,
    pub connection: Bound<'py, PyAny>,
    pub execute: Bound<'py, PyAny>,
    pub selected_keys: Bound<'py, PyAny>,
    pub full_refresh: bool,
    pub full_refresh_model_names: Bound<'py, PyAny>,
    pub on_progress: Bound<'py, PyAny>,
    pub deferred_locations: Bound<'py, PyAny>,
    pub cursor_scope: Bound<'py, PyAny>,
}
