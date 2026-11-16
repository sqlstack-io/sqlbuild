//! Public records of cache.

use crate::cache::_helpers::native_parse_store::NATIVE_PARSE_STORE_FILE_NAME;
use crate::cache::_helpers::native_parse_store::NATIVE_PARSE_STORE_KIND;
use crate::cache::_helpers::native_parse_store::NATIVE_PARSE_STORE_VERSION;
use crate::cache::_helpers::native_store::current_code_identity;
use crate::cache::_helpers::native_store::open_store;
use crate::cache::_helpers::native_store::save_store;
use crate::cache::_helpers::sql_test_scan_store::SQL_TEST_SCAN_STORE_FILE_NAME;
use crate::cache::_helpers::sql_test_scan_store::SQL_TEST_SCAN_STORE_KIND;
use crate::cache::_helpers::sql_test_scan_store::SQL_TEST_SCAN_STORE_VERSION;
use crate::cache::_helpers::sql_test_scan_store::debug;
use crate::cache::_helpers::sql_test_scan_store::poisoned;
use pyo3::prelude::Py;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use sqlbuild_cache::digest::main::content_digest::content_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// The statements this build's native parsers read whole, by dialect, SQL and placeholders.
#[derive(Debug)]
pub struct NativeParseStore {
    pub(crate) store: NativeStore,
    pub(crate) path: PathBuf,
}

impl NativeParseStore {
    /// The compile cache folder's store; none when it cannot open, which is logged for debugging.
    pub fn open_in(py: Python<'_>, cache_dir: &Path) -> PyResult<Option<Self>> {
        let path: PathBuf = cache_dir.join(NATIVE_PARSE_STORE_FILE_NAME);
        let identity: String = current_code_identity(py)?;
        let environment: String =
            hex_digest(&content_digest(&[NATIVE_PARSE_STORE_VERSION, &identity]));
        match open_store(py, &path, NATIVE_PARSE_STORE_KIND, &environment) {
            Ok(store) => Ok(Some(Self { store, path })),
            Err(error) => {
                debug(py, "native parse store not loaded: %s", &error.to_string())?;
                Ok(None)
            }
        }
    }

    /// Whether the parser read the statement under `key` whole in an earlier compile.
    pub fn read_whole(&mut self, key: &ContentDigest) -> bool {
        self.store.get(key).is_some()
    }

    /// Record that the parser read the statement under `key` whole.
    pub fn record(&mut self, key: ContentDigest) {
        self.store.put(key, Vec::new());
    }

    /// Atomically save verdicts this compile recorded; a failed save is logged for debugging.
    pub fn save(&self, py: Python<'_>) -> PyResult<()> {
        if let Err(error) = save_store(py, &self.store, &self.path, &[]) {
            debug(py, "native parse store not saved: %s", &error.to_string())?;
        }
        Ok(())
    }
}

/// One compile's view of the stored scan results, keyed by the digest of the caller's key parts.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct SqlTestScanStore {
    pub(crate) store: Mutex<NativeStore>,
    pub(crate) path: PathBuf,
    pub(crate) hits: AtomicUsize,
    pub(crate) misses: AtomicUsize,
}

#[pymethods]
impl SqlTestScanStore {
    /// Open the compile cache folder's store for `code_identity`; the error when it cannot open.
    #[staticmethod]
    pub(crate) fn open_in(
        py: Python<'_>,
        cache_dir: PathBuf,
        code_identity: &str,
    ) -> (Option<Self>, Option<String>) {
        let path: PathBuf = cache_dir.join(SQL_TEST_SCAN_STORE_FILE_NAME);
        let environment: String = hex_digest(&content_digest(&[
            SQL_TEST_SCAN_STORE_VERSION,
            code_identity,
        ]));
        match open_store(py, &path, SQL_TEST_SCAN_STORE_KIND, &environment) {
            Ok(store) => (
                Some(Self {
                    store: Mutex::new(store),
                    path,
                    hits: AtomicUsize::new(0),
                    misses: AtomicUsize::new(0),
                }),
                None,
            ),
            Err(error) => (None, Some(error.to_string())),
        }
    }

    /// The hits and misses counted by this compile.
    pub(crate) fn counts(&self) -> (usize, usize) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Atomically save values this compile stored; entries written, or the error that stopped it.
    pub(crate) fn save(&self, py: Python<'_>) -> PyResult<(Option<usize>, Option<String>)> {
        let store = self.store.lock().map_err(poisoned)?;
        Ok(match save_store(py, &store, &self.path, &[]) {
            Ok(written) => (written, None),
            Err(error) => (None, Some(error.to_string())),
        })
    }
}

impl SqlTestScanStore {
    /// The stored value under `key_parts`, counted as a hit when `decode` accepts it.
    pub fn decoded<T>(
        &self,
        key_parts: Vec<String>,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> PyResult<Option<T>> {
        let key: ContentDigest = content_digest(&key_parts);
        let decoded: Option<T> = self
            .store
            .lock()
            .map_err(poisoned)?
            .get(&key)
            .and_then(decode);
        let counter: &AtomicUsize = if decoded.is_some() {
            &self.hits
        } else {
            &self.misses
        };
        counter.fetch_add(1, Ordering::Relaxed);
        Ok(decoded)
    }

    /// Store one complete result under `key_parts`.
    pub fn stored(&self, key_parts: Vec<String>, value: Vec<u8>) -> PyResult<()> {
        let key: ContentDigest = content_digest(&key_parts);
        self.store.lock().map_err(poisoned)?.put(key, value);
        Ok(())
    }
}

/// The compile cache folder's scan store, opened on first use and saved once.
#[pyclass(module = "sqlbuild._native", frozen)]
#[derive(Debug)]
pub struct SqlTestScanCache {
    pub(crate) state: Mutex<(Option<PathBuf>, Option<Py<SqlTestScanStore>>)>,
}

#[pymethods]
impl SqlTestScanCache {
    #[new]
    #[pyo3(signature = (*, cache_dir))]
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        Self {
            state: Mutex::new((cache_dir, None)),
        }
    }

    /// The native store this compile reads and writes, or None when it has none.
    pub(crate) fn opened(&self, py: Python<'_>) -> PyResult<Option<Py<SqlTestScanStore>>> {
        let mut state = self.state.lock().map_err(poisoned)?;
        let (cache_dir, store) = &mut *state;
        let Some(directory) = cache_dir.as_ref().filter(|_| store.is_none()) else {
            return Ok(store.as_ref().map(|store| store.clone_ref(py)));
        };
        let identity: String = current_code_identity(py)?;
        let (opened, error) = SqlTestScanStore::open_in(py, directory.clone(), &identity);
        *store = opened.map(|opened| Py::new(py, opened)).transpose()?;
        if let Some(error) = error {
            debug(py, "SQL test scan store not loaded: %s", &error)?;
            *cache_dir = None;
        }
        Ok(store.as_ref().map(|store| store.clone_ref(py)))
    }

    /// Persist results stored by this compile and report hit and miss counts.
    pub(crate) fn save(&self, py: Python<'_>) -> PyResult<()> {
        let store: Option<Py<SqlTestScanStore>> = self
            .state
            .lock()
            .map_err(poisoned)?
            .1
            .as_ref()
            .map(|store| store.clone_ref(py));
        let (hits, misses) = store.as_ref().map_or((0, 0), |store| store.get().counts());
        record_compile_metric(
            "sql_test_scan_cache_hits",
            i64::try_from(hits).unwrap_or(i64::MAX),
        );
        record_compile_metric(
            "sql_test_scan_cache_misses",
            i64::try_from(misses).unwrap_or(i64::MAX),
        );
        let Some(store) = store else {
            return Ok(());
        };
        if let (_, Some(error)) = store.get().save(py)? {
            debug(py, "SQL test scan store not saved: %s", &error)?;
        }
        Ok(())
    }
}
