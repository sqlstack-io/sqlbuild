//! A verified cached index, or `None` on any cache fault.

use crate::lineage::_helpers::lineage::lineage_cache::CACHE_ALGORITHM_VERSION;
use crate::lineage::_helpers::lineage::lineage_cache::CACHE_MAX_BYTES;
use crate::lineage::_helpers::lineage::lineage_cache::CACHE_SCHEMA_VERSION;
use crate::lineage::_helpers::lineage::lineage_cache::cache_path;
use crate::lineage::_helpers::lineage::lineage_cache::connect;
use crate::lineage::_helpers::lineage::lineage_cache::read_index;
use crate::lineage::_helpers::lineage::lineage_cache::rows;
use crate::lineage::_helpers::lineage::lineage_cache::skips_cache;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::lineage_command::models::LineageIndex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A verified cached index, or `None` on any cache fault.
pub fn read_lineage_cache(
    py: Python<'_>,
    project_dir: &Path,
    fingerprint: &str,
) -> PyResult<Option<LineageIndex>> {
    let path: PathBuf = cache_path(project_dir);
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.len() <= CACHE_MAX_BYTES => {}
        _ => return Ok(None),
    }
    let read = || -> PyResult<Option<LineageIndex>> {
        let connection: Bound<'_, PyAny> =
            connect(py, &format!("file:{}?mode=ro", path.display()), true)?;
        let result = (|| -> PyResult<Option<LineageIndex>> {
            let metadata: Vec<(String, String)> =
                rows(&connection, "SELECT key, value FROM metadata")?;
            let mut metadata: HashMap<String, String> = metadata.into_iter().collect();
            let expected: [(&str, &str); 3] = [
                ("algorithm_version", CACHE_ALGORITHM_VERSION),
                ("input_fingerprint", fingerprint),
                ("schema_version", CACHE_SCHEMA_VERSION),
            ];
            let matches: bool = metadata.len() == expected.len()
                && expected
                    .iter()
                    .all(|(key, value)| metadata.remove(*key).as_deref() == Some(*value));
            if !matches {
                return Ok(None);
            }
            read_index(&connection)
        })();
        connection.call_method0("close")?;
        result
    };
    match read() {
        Ok(index) => Ok(index),
        Err(error) if skips_cache(py, &error)? => Ok(None),
        Err(error) => Err(error),
    }
}
