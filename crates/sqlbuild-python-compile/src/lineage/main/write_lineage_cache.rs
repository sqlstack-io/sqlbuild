//! Publish `index` atomically; a failure only skips the cache.

use crate::lineage::_helpers::lineage::lineage_cache::cache_path;
use crate::lineage::_helpers::lineage::lineage_cache::connect;
use crate::lineage::_helpers::lineage::lineage_cache::skips_cache;
use crate::lineage::_helpers::lineage::lineage_cache::write_index;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::lineage_command::models::LineageIndex;
use std::path::{Path, PathBuf};

/// Publish `index` atomically; a failure only skips the cache.
pub fn write_lineage_cache(
    py: Python<'_>,
    project_dir: &Path,
    fingerprint: &str,
    index: &LineageIndex,
) -> PyResult<()> {
    let path: PathBuf = cache_path(project_dir);
    let Some(directory) = path.parent() else {
        return Ok(());
    };
    if std::fs::create_dir_all(directory).is_err() {
        return Ok(());
    }
    let mut temporary: Option<PathBuf> = None;
    let mut write = || -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("prefix", ".structural-graph-")?;
        options.set_item("dir", directory)?;
        let (descriptor, name): (i64, PathBuf) = py
            .import("tempfile")?
            .getattr("mkstemp")?
            .call((), Some(&options))?
            .extract()?;
        py.import("os")?.getattr("close")?.call1((descriptor,))?;
        temporary = Some(name.clone());
        let connection: Bound<'_, PyAny> = connect(py, &name.to_string_lossy(), false)?;
        let written: PyResult<()> = write_index(&connection, fingerprint, index);
        connection.call_method0("close")?;
        written?;
        std::fs::rename(&name, &path)?;
        Ok(())
    };
    let outcome: PyResult<()> = write();
    if let Some(name) = temporary {
        let _ = std::fs::remove_file(name);
    }
    match outcome {
        Err(error) if !skips_cache(py, &error)? => Err(error),
        _ => Ok(()),
    }
}
