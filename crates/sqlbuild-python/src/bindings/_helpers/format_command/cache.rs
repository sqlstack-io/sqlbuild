//! Files already formatted and fault-free, kept in the shared native store to skip next time.

use std::path::{Path, PathBuf};

use pyo3::exceptions::PyOSError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cache::digest::main::content_digest::content_digest;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::models::NativeStore;

use crate::bindings::_helpers::compile_command::constants::TARGET_DIRECTORY_NAME;
use crate::bindings::_helpers::frontier::constants::{
    CACHE_DIRECTORY_NAME, COMPILER_CACHE_DIRECTORY_NAME, NATIVE_CACHE_NAMESPACE_SUFFIX,
};
use sqlbuild_python_base::cache::main::current_code_identity::current_code_identity;
use sqlbuild_python_base::cache::main::open_store::open_store;
use sqlbuild_python_base::cache::main::save_store::save_store;

/// The native store kind holding clean format results.
const FORMAT_RESULT_STORE_KIND: &str = "format-results";
/// The store file below the compile cache folder.
pub(crate) const FORMAT_RESULT_STORE_FILE_NAME: &str = "format-results.bin";
/// Bumped whenever what a clean result means changes; older stores open empty.
const FORMAT_RESULT_STORE_VERSION: &str = "format-result-store-v1";
/// The one value stored: formatting left the file unchanged and reported nothing about it.
const CLEAN: &[u8] = b"clean";

/// One run's view of the stored clean results, keyed by file, text and effective settings.
pub(crate) struct FormatCache {
    store: NativeStore,
    path: PathBuf,
    settings: String,
}

impl FormatCache {
    /// The project's store for this SQLBuild build, or `None` when it cannot be read.
    pub(crate) fn open(
        py: Python<'_>,
        project_dir: &Path,
        settings: String,
    ) -> PyResult<Option<Self>> {
        let path: PathBuf = project_dir
            .join(TARGET_DIRECTORY_NAME)
            .join(CACHE_DIRECTORY_NAME)
            .join(format!(
                "{COMPILER_CACHE_DIRECTORY_NAME}{NATIVE_CACHE_NAMESPACE_SUFFIX}"
            ))
            .join(FORMAT_RESULT_STORE_FILE_NAME);
        let environment: String = hex_digest(&content_digest(&[
            FORMAT_RESULT_STORE_VERSION,
            &current_code_identity(py)?,
        ]));
        match open_store(py, &path, FORMAT_RESULT_STORE_KIND, &environment) {
            Ok(store) => Ok(Some(Self {
                store,
                path,
                settings,
            })),
            Err(error) if error.is_instance_of::<PyOSError>(py) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The key of one file: its project-relative path, its exact text and the run's settings.
    pub(crate) fn key(&self, relative_path: &str, contents: &str) -> ContentDigest {
        content_digest(&[relative_path, contents, &self.settings])
    }

    /// Whether a previous run found the file under `key` clean.
    pub(crate) fn is_clean(&mut self, key: &ContentDigest) -> bool {
        self.store.get(key) == Some(CLEAN)
    }

    /// Remember that the file under `key` formatted cleanly.
    pub(crate) fn record_clean(&mut self, key: ContentDigest) {
        self.store.put(key, CLEAN.to_vec());
    }

    /// Save newly recorded results; a store that cannot be written is left as it was.
    pub(crate) fn save(&self, py: Python<'_>) {
        let _ = save_store(py, &self.store, &self.path, &[]);
    }
}
