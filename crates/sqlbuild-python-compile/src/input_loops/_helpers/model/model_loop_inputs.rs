//! Reading a model's config values the way the attachment reads them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyBoolMethods, PyDict, PyDictMethods};
use sqlbuild_analysis::compiled_project::main::header_metadata_keys::is_header_metadata_key;
use sqlbuild_analysis::compiled_project::models::{AnalysisOverride, ModelReferenceFact};
use sqlbuild_sqltext::sql_references::models::SqlReference;

use sqlbuild_project::models::ReferenceRow;

const SQL_ANALYSIS_KEY: &str = "sql_analysis";
const SQL_VALIDATION_KEY: &str = "sql_validation";
pub(crate) const HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];

/// Whether either hook key holds a value.
pub(crate) fn hook_presence(values: &Bound<'_, PyDict>) -> PyResult<bool> {
    for key in HOOK_KEYS {
        if present(values, key)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The override `_is_sql_validation_enabled` reads: unequal keys conflict, a non-bool inherits.
pub(crate) fn analysis_override(values: &Bound<'_, PyDict>) -> PyResult<AnalysisOverride> {
    let analysis: Option<Bound<'_, PyAny>> = present(values, SQL_ANALYSIS_KEY)?;
    let legacy: Option<Bound<'_, PyAny>> = present(values, SQL_VALIDATION_KEY)?;
    let chosen: Option<Bound<'_, PyAny>> = match (analysis, legacy) {
        (Some(analysis), Some(legacy)) => {
            if analysis.ne(&legacy)? {
                return Ok(AnalysisOverride::Conflict);
            }
            Some(analysis)
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    };
    Ok(
        match chosen.map(pyo3::types::PyAnyMethods::downcast_into::<PyBool>) {
            Some(Ok(flag)) if flag.is_true() => AnalysisOverride::Enabled,
            Some(Ok(_)) => AnalysisOverride::Disabled,
            Some(Err(_)) | None => AnalysisOverride::Inherit,
        },
    )
}

/// The value under `key`, treating an explicit `None` as absent.
pub(crate) fn present<'py>(
    values: &Bound<'py, PyDict>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(values.get_item(key)?.filter(|value| !value.is_none()))
}

/// The config without header metadata, always a new dictionary as hook expansion leaves it.
pub(crate) fn stripped_values<'py>(
    py: Python<'py>,
    values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let stripped: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in values.iter() {
        if !is_header_metadata_key(&key.str()?.to_string()) {
            stripped.set_item(key, value)?;
        }
    }
    Ok(stripped)
}

pub(crate) fn reference_row(reference: &SqlReference) -> ReferenceRow {
    ReferenceRow {
        kind: reference.kind.to_owned(),
        name: reference.name.clone(),
        package: reference.package.clone(),
        call_argument_count: reference.call_argument_count,
    }
}

pub(crate) fn reference_fact(
    ReferenceRow {
        kind,
        name,
        package,
        ..
    }: ReferenceRow,
) -> ModelReferenceFact {
    ModelReferenceFact {
        kind,
        name,
        package,
    }
}
