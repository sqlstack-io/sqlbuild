//! Load or build scope facts without credentials, a connection, or warehouse access.

use crate::scopes::_helpers::scope_loading::cache_location;
use crate::scopes::_helpers::scope_loading::compiled_index;
use crate::scopes::_helpers::scope_loading::is_project_input_error;
use crate::scopes::_helpers::scope_loading::project_adapter;
use crate::scopes::_helpers::scope_loading::tolerant_index;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::scopes::main::index::project_fingerprint::project_fingerprint;
use sqlbuild_python_front::scopes::main::index::read_scope_index_cache::read_scope_index_cache;
use sqlbuild_python_front::scopes::main::index::write_scope_index_cache::write_scope_index_cache;

/// Load or build scope facts without credentials, a connection, or warehouse access.
#[pyfunction]
#[pyo3(signature = (*, project_dir, no_cache=false))]
pub fn load_or_build_scope_index<'py>(
    project_dir: &Bound<'py, PyAny>,
    no_cache: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let resolved: Bound<'py, PyAny> = project_dir.call_method0("resolve")?;
    let fingerprint: Option<String> = project_fingerprint(py, resolved.extract()?)?;
    let cached_fingerprint: Option<&String> = fingerprint.as_ref().filter(|_| !no_cache);
    if let Some(fingerprint) = cached_fingerprint {
        let (directory, file_name) = cache_location(&resolved)?;
        if let Some(cached) =
            read_scope_index_cache(py, directory.join(file_name), fingerprint.clone())?
        {
            return Ok(cached.into_bound(py));
        }
    }
    let mut syntax: Bound<'py, PyAny> = new_record(py, RecordId::SqlLexicalSyntax, (), None)?;
    let compiled: PyResult<Bound<'py, PyAny>> = match project_adapter(&resolved) {
        Ok((discovered, adapter)) => {
            syntax = native_field(&adapter, "sql_lexical_syntax")?;
            compiled_index(discovered, &adapter, &syntax)
        }
        Err(error) => Err(error),
    };
    let index: Bound<'py, PyAny> = match compiled {
        Ok(index) => index,
        Err(error) if is_project_input_error(py, &error) => tolerant_index(&resolved, &syntax)?,
        Err(error) => return Err(error),
    };
    if let Some(fingerprint) = cached_fingerprint {
        let _ =
            write_scope_index_cache(py, cache_location(&resolved)?, fingerprint.clone(), &index)?;
    }
    Ok(index)
}
