//! The module file digests a macro call store records and checks before reuse.

use crate::macro_bridge::main::text::text;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyList, PyListMethods, PyString};

use sqlbuild_cache::module_digests::main::module_digests_metadata::module_digests_metadata as metadata_text;
use sqlbuild_core::json::models::JsonValue;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::cache::main::digest_files::digest_files;
use sqlbuild_python_base::frontier::main::emit_error::emit_error;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// The fields of one stored row: a path and its digest.
const MODULE_DIGEST_ROW_FIELDS: usize = 2;

/// Content digest of each module file, or None when any file cannot be read.
#[pyfunction]
pub(crate) fn digest_module_files<'py>(
    py: Python<'py>,
    paths: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let ordered: Bound<'py, PyDict> = PyDict::new(py);
    for path in paths.try_iter()? {
        ordered.set_item(path?, py.None())?;
    }
    let mut files: Vec<PathBuf> = Vec::with_capacity(ordered.len());
    for path in ordered.keys() {
        files.push(path.extract()?);
    }
    let digests: Vec<Option<String>> = digest_files(py, files)?;
    if digests.iter().any(Option::is_none) {
        return Ok(None);
    }
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for (path, digest) in ordered.keys().iter().zip(digests) {
        if let Some(digest) = digest.filter(|digest| !digest.is_empty()) {
            result.set_item(path, digest)?;
        }
    }
    Ok(Some(result))
}

/// Encode module file digests for the store's metadata.
#[pyfunction]
pub(crate) fn module_digests_metadata<'py>(
    py: Python<'py>,
    digests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut rows: Vec<(JsonValue, JsonValue)> = Vec::new();
    for item in mapping_items(digests)?.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        rows.push((
            text(item.get_item(0)?.downcast::<PyString>()?)?,
            text(item.get_item(1)?.downcast::<PyString>()?)?,
        ));
    }
    let encoded: String = metadata_text(rows).map_err(emit_error)?;
    Ok(PyBytes::new(py, encoded.as_bytes()))
}

/// Whether a decoded metadata row is a list of a path and a digest.
fn digest_row_check(row: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Ok(row) = row.downcast::<PyList>() else {
        return Ok(false);
    };
    if row.len() != MODULE_DIGEST_ROW_FIELDS {
        return Ok(false);
    }
    for field in row.iter() {
        if !field.is_instance_of::<PyString>() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Return the stored module digests when every module file still has them, else None.
#[pyfunction]
#[pyo3(signature = (*, metadata, known))]
pub(crate) fn unchanged_module_digests<'py>(
    py: Python<'py>,
    metadata: &Bound<'py, PyAny>,
    known: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let rows: Bound<'py, PyAny> = match imported(py, "json", "loads")?.call1((metadata,)) {
        Ok(rows) => rows,
        Err(error) if error.is_instance_of::<PyValueError>(py) => return Ok(None),
        Err(error) => return Err(error),
    };
    let Ok(rows) = rows.downcast::<PyList>() else {
        return Ok(None);
    };
    for row in rows.iter() {
        if !digest_row_check(&row)? {
            return Ok(None);
        }
    }
    let stored: Bound<'py, PyDict> = PyDict::new(py);
    for row in rows.iter() {
        stored.set_item(row.get_item(0)?, row.get_item(1)?)?;
    }
    let mut unknown: Vec<Bound<'py, PyAny>> = Vec::new();
    for path in stored.keys() {
        if !known.contains(&path)? {
            unknown.push(path);
        }
    }
    let Some(current) = digest_module_files(py, PyList::new(py, unknown)?.as_any())? else {
        return Ok(None);
    };
    for path in stored.keys() {
        if known.contains(&path)? {
            current.set_item(&path, known.get_item(&path)?)?;
        }
    }
    Ok(if current.eq(&stored)? {
        Some(stored)
    } else {
        None
    })
}
