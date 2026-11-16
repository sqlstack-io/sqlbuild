//! Whole per-file SQL-test scan results kept as one kind of the shared native store.

use crate::cache::models::SqlTestScanCache;
use crate::cache::models::SqlTestScanStore;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};

use crate::boundary::main::imported::imported;

const SCAN_CACHE_LOGGER: &str = "sqlbuild.compile";

/// The native store kind holding SQL-test scan results.
pub(crate) const SQL_TEST_SCAN_STORE_KIND: &str = "sql-test-scans";
/// The store file below the compile cache folder.
pub(crate) const SQL_TEST_SCAN_STORE_FILE_NAME: &str = "sql-test-scans.bin";
/// Bumped whenever stored scan values change shape; older stores open empty.
pub(crate) const SQL_TEST_SCAN_STORE_VERSION: &str = "sql-test-scan-store-v1";

pub(crate) fn poisoned<T>(_: T) -> pyo3::PyErr {
    PyValueError::new_err("SQL test scan store is poisoned")
}

/// `message` with `error` at debug level on the compile logger, as the compile caches report.
pub(crate) fn debug(py: Python<'_>, message: &str, error: &str) -> PyResult<()> {
    imported(py, "logging", "getLogger")?
        .call1((SCAN_CACHE_LOGGER,))?
        .call_method1("debug", (message, error))?;
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<SqlTestScanStore>()?;
    module.add_class::<SqlTestScanCache>()?;
    module.add(
        "SQL_TEST_SCAN_STORE_FILE_NAME",
        SQL_TEST_SCAN_STORE_FILE_NAME,
    )?;
    Ok(())
}
