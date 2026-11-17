//! Migration fingerprints computed at most once per model definition and reused across runs.

use crate::bindings::_helpers::planner_changes::metadata::without_declared_columns_hash;
use crate::bindings::_helpers::planner_migrations::fingerprint::fingerprint;
use crate::bindings::_helpers::planner_migrations::fingerprint::migration_fingerprint_ref_names;
use crate::bindings::_helpers::planner_warehouse::models::FingerprintWork;
use pyo3::PyRefMut;
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyFloat, PyList, PyListMethods, PyTuple};
use pyo3::{pyclass, pyfunction, pymethods};
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_entry::fingerprint_cache_entry;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_key::fingerprint_cache_key;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_lookup::fingerprint_cache_lookup;
use sqlbuild_analysis::planner_warehouse::main::fingerprint_cache_shared_digest::fingerprint_cache_shared_digest;
use sqlbuild_analysis::planner_warehouse::models::FingerprintCacheKey;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::object::object;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

const CACHE_DIRECTORY_NAME: &str = "migration-fingerprints-v1";
const CACHE_DATABASE_NAME: &str = "migration-fingerprints.sqlite3";
const SQLITE_TIMEOUT_SECONDS: f64 = 0.1;
const CREATE_CACHE_TABLE_SQL: &str = "\nCREATE TABLE IF NOT EXISTS migration_fingerprint (\n    cache_key TEXT PRIMARY KEY,\n    payload TEXT NOT NULL\n)\n";
const INSERT_SQL: &str =
    "INSERT OR REPLACE INTO migration_fingerprint (cache_key, payload) VALUES (?, ?)";
const SELECT_SQL: &str = "SELECT payload FROM migration_fingerprint WHERE cache_key = ?";

/// A fingerprint request: query SQL, metadata JSON, dialect and visible renames.
#[derive(Hash, PartialEq, Eq)]
struct MemoKey {
    query_sql: String,
    metadata_json: String,
    dialect: Option<String>,
    visible_renames: Vec<(String, String)>,
}

static FINGERPRINT_WORK: PyOnceLock<Py<FingerprintWork>> = PyOnceLock::new();

fn fingerprint_work(py: Python<'_>) -> PyResult<PyRefMut<'_, FingerprintWork>> {
    Ok(FINGERPRINT_WORK
        .get_or_try_init(py, || Py::new(py, FingerprintWork::default()))?
        .bind(py)
        .borrow_mut())
}

fn count_computation(py: Python<'_>, key: &MemoKey) -> PyResult<()> {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let mut work = fingerprint_work(py)?;
    work.computations += 1;
    work.distinct.insert(hasher.finish());
    Ok(())
}

/// Report migration fingerprints computed and distinct requests among them, optionally resetting.
#[pyfunction]
#[pyo3(signature = (*, reset = false))]
pub(crate) fn migration_fingerprint_work_info(
    py: Python<'_>,
    reset: bool,
) -> PyResult<(u64, usize)> {
    let mut work = fingerprint_work(py)?;
    let info = (work.computations, work.distinct.len());
    if reset {
        *work = FingerprintWork::default();
    }
    Ok(info)
}

/// Reuse migration fingerprints across rename maps, planning phases and invocations.
#[pyclass(module = "sqlbuild._native")]
pub(crate) struct MigrationFingerprintCache {
    ref_names: HashMap<(String, String), Vec<String>>,
    fingerprints: HashMap<MemoKey, Option<String>>,
    database_path: Option<Py<PyAny>>,
    store: Option<Py<PyAny>>,
    store_opened: bool,
    pending: Vec<(String, String)>,
    shared_digest: String,
}

fn sqlite_error(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &imported(py, "sqlite3", "DatabaseError")?))
}

fn close(py: Python<'_>, store: Option<Py<PyAny>>) -> PyResult<()> {
    let Some(store) = store else {
        return Ok(());
    };
    match store.bind(py).call_method0("close") {
        Ok(_) => Ok(()),
        Err(error) if sqlite_error(py, &error)? => Ok(()),
        Err(error) => Err(error),
    }
}

fn connect<'py>(path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = path.py();
    call_with(
        &imported(py, "sqlite3", "connect")?,
        &[
            ("database", path.clone()),
            (
                "timeout",
                PyFloat::new(py, SQLITE_TIMEOUT_SECONDS).into_any(),
            ),
        ],
    )
}

fn package_version(py: Python<'_>, package: &str) -> PyResult<String> {
    match imported(py, "importlib.metadata", "version")?.call1((package,)) {
        Ok(version) => version.extract(),
        Err(error)
            if error.is_instance(
                py,
                &imported(py, "importlib.metadata", "PackageNotFoundError")?,
            ) =>
        {
            Ok("unknown".to_owned())
        }
        Err(error) => Err(error),
    }
}

impl MigrationFingerprintCache {
    /// A cache persisting under `root`, or an in-memory cache without one.
    pub(crate) fn open(py: Python<'_>, root: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
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
