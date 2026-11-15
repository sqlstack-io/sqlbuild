//! Audit SQL with its reference calls replaced by relation names or overrides.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::auditing::main::replace_reference_calls::replace_reference_calls;

use crate::bindings::_helpers::references::source_relations::{
    render_source_read, render_source_relation,
};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

const REF_FUNCTION: &str = "__ref";
const SEED_FUNCTION: &str = "__seed";
const SOURCE_FUNCTION: &str = "__source";

/// `mapping.get(name)`, or `None` when absent or `None`.
fn lookup<'py>(mapping: &Bound<'py, PyAny>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = mapping_get(mapping, name)?;
    Ok(Some(value).filter(|value| !value.is_none()))
}

/// The qualified name of the relation location `mapping` holds for `name`, if any.
fn qualified<'py>(mapping: &Bound<'py, PyAny>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(location) = lookup(mapping, name)? else {
        return Ok(None);
    };
    let qualified: Bound<'py, PyAny> = native_field(&location, "qualified_name")?;
    Ok(Some(qualified).filter(|qualified| !qualified.is_none()))
}

/// Render audit SQL from unresolved markers, applying relation overrides first.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn render_audit_sql(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let request = Request::new("render_audit_sql", request)?;
    let sql: String = request.get("unresolved_sql")?.extract()?;
    let models: Bound<'_, PyAny> = request.get("model_locations")?;
    let seeds: Bound<'_, PyAny> = request.get("seed_locations")?;
    let sources: Bound<'_, PyAny> = request.get("source_map")?;
    let adapter: Option<Bound<'_, PyAny>> = request
        .optional("adapter")?
        .filter(|adapter| !adapter.is_none());
    let overrides: Option<Bound<'_, PyAny>> = request
        .optional("relation_overrides")?
        .filter(|overrides| !overrides.is_none());
    let resolved: String = replace_reference_calls(&sql, REF_FUNCTION, |name| {
        if let Some(overrides) = &overrides
            && let Some(relation) = lookup(overrides, name)?
        {
            return relation.extract().map(Some);
        }
        let location: Option<Bound<'_, PyAny>> = match lookup(&models, name)? {
            Some(_) => qualified(&models, name)?,
            None => qualified(&seeds, name)?,
        };
        location.map(|name| name.extract()).transpose()
    })?;
    let resolved: String = replace_reference_calls(&resolved, SEED_FUNCTION, |name| {
        qualified(&seeds, name)?
            .map(|name| name.extract())
            .transpose()
    })?;
    replace_reference_calls(&resolved, SOURCE_FUNCTION, |name| {
        let Some(entry) = lookup(&sources, name)? else {
            return Ok(None);
        };
        match adapter.as_ref() {
            Some(adapter) => render_source_read(&entry, adapter).map(Some),
            None => render_source_relation(&entry, None).map(Some),
        }
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_audit_sql, module)?)?;
    Ok(())
}
