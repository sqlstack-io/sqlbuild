//! Render audit SQL from unresolved markers, applying relation overrides first.

use crate::auditing::_helpers::render::REF_FUNCTION;
use crate::auditing::_helpers::render::SEED_FUNCTION;
use crate::auditing::_helpers::render::SOURCE_FUNCTION;
use crate::auditing::_helpers::render::lookup;
use crate::auditing::_helpers::render::qualified;
use crate::references::main::render_source_read::render_source_read;
use crate::references::main::render_source_relation::render_source_relation;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_analysis::auditing::main::replace_reference_calls::replace_reference_calls;
use sqlbuild_python_base::boundary::models::Request;

/// Render audit SQL from unresolved markers, applying relation overrides first.
#[pyfunction]
#[pyo3(signature = (**request))]
pub fn render_audit_sql(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
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
