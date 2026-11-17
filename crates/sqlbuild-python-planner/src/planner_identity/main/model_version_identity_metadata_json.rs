//! Deterministic non-query model fingerprint metadata JSON.

use crate::planner_identity::_helpers::metadata::execution_signature;
use crate::planner_identity::_helpers::metadata::version_identity_metadata_json;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_identity::constants::{TABLE_FN_RESOURCE, UDF_RESOURCE};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::compiled_views::main::compiled_models::config::config;
use sqlbuild_python_project::compiled_views::main::compiled_models::deps::deps;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;

/// Deterministic non-query model fingerprint metadata JSON.
pub fn model_version_identity_metadata_json<'py>(
    model: &Bound<'py, PyAny>,
    function_local_hashes: Option<&Bound<'py, PyAny>>,
    hook_version_hashes: Option<&Bound<'py, PyAny>>,
) -> PyResult<String> {
    let py: Python<'py> = model.py();
    let local_function_hashes: Bound<'py, PyDict> = PyDict::new(py);
    if let Some(function_hashes) = function_local_hashes.filter(|hashes| !hashes.is_none()) {
        for upstream_key in deps(model)?.try_iter()? {
            let upstream_key: Bound<'py, PyAny> = upstream_key?;
            let kind: String = native_field(&upstream_key, "resource_type")?
                .str()?
                .extract()?;
            if kind != UDF_RESOURCE && kind != TABLE_FN_RESOURCE {
                continue;
            }
            let name: Bound<'py, PyAny> = native_field(&upstream_key, "name")?;
            let upstream_hash: Bound<'py, PyAny> = mapping_get(function_hashes, &name)?;
            if !upstream_hash.is_none() {
                local_function_hashes.set_item(name, upstream_hash)?;
            }
        }
    }
    let config_values: Bound<'py, PyAny> = native_field(&config(model)?, "values")?;
    let signature: Bound<'py, PyDict> =
        execution_signature(model, &config_values, hook_version_hashes)?;
    version_identity_metadata_json(
        &name_object(model)?,
        &config_values,
        (
            Some(local_function_hashes.into_any()),
            Some(signature.into_any()),
        ),
    )
}
