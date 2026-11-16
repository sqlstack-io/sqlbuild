//! Digests, project fingerprints and store files of the shared native store, with the GIL released.

use pyo3::exceptions::{PyOSError, PyRuntimeError};
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cache::digest::main::compiled_code_identity::compiled_code_identity as compiled;
use sqlbuild_cache::digest::main::content_digest::content_digest as digest;
use sqlbuild_cache::digest::main::digest_files::digest_files as digest_each;
use sqlbuild_cache::digest::main::fingerprint_project_files::fingerprint_project_files as fingerprint;
use sqlbuild_cache::digest::main::hex_digest::hex_digest;
use sqlbuild_cache::digest::main::installed_code_facts::installed_code_facts;
use sqlbuild_cache::digest::main::installed_code_identity::installed_code_identity as installed;
use sqlbuild_cache::digest::types::ContentDigest;
use sqlbuild_cache::store::main::open_native_store::open_native_store;
use sqlbuild_cache::store::models::NativeStore;
use sqlbuild_host::entries::models::FacadeHost;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::native_host::main::host_entry::host_entry;
use crate::records::main::registry::native_attr::native_attr;
use crate::records::types::NativeAttr;
use crate::types::CompilerDetach;

/// Hexadecimal digest of length-prefixed UTF-8 `parts`, the digest every native cache key uses.
#[pyfunction]
pub(crate) fn content_digest(parts: Vec<String>) -> String {
    hex_digest(&digest(&parts))
}

/// Digest of every project file outside skipped folders and `excluded_files`, else `OSError`.
#[pyfunction]
pub(crate) fn fingerprint_project_files(
    py: Python<'_>,
    root: PathBuf,
    excluded_files: Vec<String>,
) -> PyResult<String> {
    let excluded: HashSet<String> = excluded_files.into_iter().collect();
    let fingerprinted: ContentDigest = py
        .compiler_detach(|| Ok(fingerprint(&root, &excluded)))
        .map_err(PyRuntimeError::new_err)?
        .map_err(|error| PyOSError::new_err(error.to_string()))?;
    Ok(hex_digest(&fingerprinted))
}

/// Hexadecimal content digest of each file in `paths`, or `None` where it cannot be read.
#[pyfunction]
pub(crate) fn digest_files(py: Python<'_>, paths: Vec<PathBuf>) -> PyResult<Vec<Option<String>>> {
    let digests: Vec<std::io::Result<ContentDigest>> = py
        .compiler_detach(|| Ok(digest_each(&paths)))
        .map_err(PyRuntimeError::new_err)?;
    Ok(digests.iter().map(readable_digest).collect())
}

fn readable_digest(digest: &std::io::Result<ContentDigest>) -> Option<String> {
    match digest {
        Ok(digest) => Some(hex_digest(digest)),
        Err(_unreadable) => None,
    }
}

/// Open one cache kind's store with the GIL released; a failed read is an `OSError`.
pub(crate) fn open_store(
    py: Python<'_>,
    path: &Path,
    kind: &str,
    environment: &str,
) -> PyResult<NativeStore> {
    py.compiler_detach(|| Ok(open_native_store(path, kind, environment)))
        .map_err(PyRuntimeError::new_err)?
        .map_err(|error| PyOSError::new_err(error.to_string()))
}

/// Atomically save a store holding unsaved values with the GIL released; returns entries written.
pub(crate) fn save_store(
    py: Python<'_>,
    store: &NativeStore,
    path: &Path,
    metadata: &[u8],
) -> PyResult<Option<usize>> {
    if !store.needs_save() {
        return Ok(None);
    }
    py.compiler_detach(|| Ok(store.save(path, metadata)))
        .map_err(PyRuntimeError::new_err)?
        .map(Some)
        .map_err(|error| PyOSError::new_err(error.to_string()))
}

/// The digest that changes with every released, locally edited, or rebuilt SQLBuild version.
#[pyfunction]
pub(crate) fn compiled_code_identity(py: Python<'_>) -> PyResult<String> {
    static INSTALLED: PyOnceLock<String> = PyOnceLock::new();
    let installed: &String = INSTALLED.get_or_try_init(py, || installed_identity(py))?;
    let build_identity: String = native_attr(py, NativeAttr::BuildIdentity)?.extract()?;
    Ok(compiled(installed, &build_identity))
}

/// `compiled_code_identity`, looked up on the extension module so tests can replace it.
pub(crate) fn current_code_identity(py: Python<'_>) -> PyResult<String> {
    native_attr(py, NativeAttr::CompiledCodeIdentity)?
        .call0()?
        .extract()
}

/// Installed distribution versions, the interpreter, and an editable install's source files.
fn installed_identity(py: Python<'_>) -> PyResult<String> {
    let search_paths: Vec<PathBuf> = py
        .import("sys")?
        .getattr("path")?
        .try_iter()?
        .map(|entry| entry?.extract::<PathBuf>())
        .collect::<PyResult<_>>()?;
    let implementation: String = py
        .import("platform")?
        .call_method0("python_implementation")?
        .extract()?;
    let (major, minor): (u32, u32) = (
        py.import("sys")?
            .getattr("version_info")?
            .getattr("major")?
            .extract()?,
        py.import("sys")?
            .getattr("version_info")?
            .getattr("minor")?
            .extract()?,
    );
    let package_file: PathBuf = host_entry(py, FacadeHost::PackageFile)?.extract()?;
    let (mut facts, editable) = py
        .compiler_detach(|| Ok(installed_code_facts(&search_paths)))
        .map_err(PyRuntimeError::new_err)?;
    facts.push(implementation);
    facts.push(format!("{major}.{minor}"));
    let package_root: Option<PathBuf> = if editable {
        Some(
            package_file
                .parent()
                .unwrap_or(&package_file)
                .canonicalize()
                .map_err(|error| PyOSError::new_err(error.to_string()))?,
        )
    } else {
        None
    };
    py.compiler_detach(|| Ok(installed(&facts, package_root.as_deref())))
        .map_err(PyRuntimeError::new_err)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compiled_code_identity, module)?)?;
    module.add_function(wrap_pyfunction!(content_digest, module)?)?;
    module.add_function(wrap_pyfunction!(fingerprint_project_files, module)?)?;
    module.add_function(wrap_pyfunction!(digest_files, module)?)?;
    Ok(())
}
