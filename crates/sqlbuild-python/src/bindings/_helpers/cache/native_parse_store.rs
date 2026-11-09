//! Statements a dialect's native parser read whole, kept so a warm compile skips parsing them again.

use std::path::{Path, PathBuf};

use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::assembly::project::models::SyntaxCheck;
use sqlbuild_cache::digest::main::content_digest::content_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::cache::native_store::{
    current_code_identity, open_store, save_store,
};

const PARSE_CACHE_LOGGER: &str = "sqlbuild.compile";
/// The native store kind holding read-whole verdicts.
const NATIVE_PARSE_STORE_KIND: &str = "native-parses";
/// The store file below the compile cache folder.
const NATIVE_PARSE_STORE_FILE_NAME: &str = "native-parses.bin";
/// Bumped whenever what a key covers changes; older stores open empty.
const NATIVE_PARSE_STORE_VERSION: &str = "native-parse-store-v1";

/// The statements this build's native parsers read whole, by dialect, SQL and placeholders.
pub(crate) struct NativeParseStore {
    store: NativeStore,
    path: PathBuf,
}

impl NativeParseStore {
    /// The compile cache folder's store; none when it cannot open, which is logged for debugging.
    pub(crate) fn open_in(py: Python<'_>, cache_dir: &Path) -> PyResult<Option<Self>> {
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
    pub(crate) fn read_whole(&mut self, key: &ContentDigest) -> bool {
        self.store.get(key).is_some()
    }

    /// Record that the parser read the statement under `key` whole.
    pub(crate) fn record(&mut self, key: ContentDigest) {
        self.store.put(key, Vec::new());
    }

    /// Atomically save verdicts this compile recorded; a failed save is logged for debugging.
    pub(crate) fn save(&self, py: Python<'_>) -> PyResult<()> {
        if let Err(error) = save_store(py, &self.store, &self.path, &[]) {
            debug(py, "native parse store not saved: %s", &error.to_string())?;
        }
        Ok(())
    }
}

/// The key `dialect`'s verdict on `check` is stored under.
pub(crate) fn parse_key(dialect: &str, check: &SyntaxCheck) -> ContentDigest {
    let mut parts: Vec<&str> = vec![dialect, &check.sql];
    for (name, value) in &check.placeholders {
        parts.push(name);
        parts.push(value);
    }
    content_digest(&parts)
}

fn debug(py: Python<'_>, message: &str, error: &str) -> PyResult<()> {
    imported(py, "logging", "getLogger")?
        .call1((PARSE_CACHE_LOGGER,))?
        .call_method1("debug", (message, error))?;
    Ok(())
}
