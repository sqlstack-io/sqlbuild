//! Migration fingerprints computed at most once per model definition and reused across runs.

use crate::planner_warehouse::models::FingerprintWork;
use pyo3::PyRefMut;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyFloat;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use std::hash::{DefaultHasher, Hash, Hasher};

pub(crate) const CACHE_DIRECTORY_NAME: &str = "migration-fingerprints-v1";
pub(crate) const CACHE_DATABASE_NAME: &str = "migration-fingerprints.sqlite3";
const SQLITE_TIMEOUT_SECONDS: f64 = 0.1;
pub(crate) const CREATE_CACHE_TABLE_SQL: &str = "\nCREATE TABLE IF NOT EXISTS migration_fingerprint (\n    cache_key TEXT PRIMARY KEY,\n    payload TEXT NOT NULL\n)\n";
pub(crate) const INSERT_SQL: &str =
    "INSERT OR REPLACE INTO migration_fingerprint (cache_key, payload) VALUES (?, ?)";
pub(crate) const SELECT_SQL: &str = "SELECT payload FROM migration_fingerprint WHERE cache_key = ?";

/// A fingerprint request: query SQL, metadata JSON, dialect and visible renames.
#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) struct MemoKey {
    pub(crate) query_sql: String,
    pub(crate) metadata_json: String,
    pub(crate) dialect: Option<String>,
    pub(crate) visible_renames: Vec<(String, String)>,
}

static FINGERPRINT_WORK: PyOnceLock<Py<FingerprintWork>> = PyOnceLock::new();

fn fingerprint_work(py: Python<'_>) -> PyResult<PyRefMut<'_, FingerprintWork>> {
    Ok(FINGERPRINT_WORK
        .get_or_try_init(py, || Py::new(py, FingerprintWork::default()))?
        .bind(py)
        .borrow_mut())
}

pub(crate) fn count_computation(py: Python<'_>, key: &MemoKey) -> PyResult<()> {
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

pub(crate) fn sqlite_error(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &imported(py, "sqlite3", "DatabaseError")?))
}

pub(crate) fn close(py: Python<'_>, connection: Option<Py<PyAny>>) -> PyResult<()> {
    let Some(connection) = connection else {
        return Ok(());
    };
    match connection.bind(py).call_method0("close") {
        Ok(_) => Ok(()),
        Err(error) if sqlite_error(py, &error)? => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn connect<'py>(path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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

pub(crate) fn package_version(py: Python<'_>, package: &str) -> PyResult<String> {
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
