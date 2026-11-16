//! Compile inputs' canonical index with all usage and placement facts.

use crate::scopes::_helpers::checks::scope_validation::COMPILE_USAGE_GROUPS;
use crate::scopes::_helpers::checks::scope_validation::scope_placement_validated_index;
use crate::scopes::_helpers::checks::scope_validation::usage_records;
use crate::scopes::_helpers::relationships::compile_usages::model_schema_scope_usages;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Compile inputs' canonical index with all usage and placement facts.
pub fn scope_index_with_compile_usages<'py>(
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = inputs.py();
    let scope: Bound<'py, PyAny> = native_field(inputs, "declaration_scope")?;
    let resolver: Bound<'py, PyAny> = if scope.is_none() {
        scope
    } else {
        native_field(&scope, "resolver")?
    };
    let schema_files: Bound<'py, PyAny> = native_field(
        &native_field(inputs, "discovered_inputs")?,
        "model_schema_files",
    )?;
    let mut usages: Vec<Bound<'py, PyAny>> = Vec::new();
    usages.extend(usage_records(
        model_schema_scope_usages(&schema_files, &resolver)?.as_any(),
    )?);
    let digests: bool = sqlbuild_python_base::compile_context::main::delta::capturing::capturing();
    for model in native_field(inputs, "model_inputs")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let macro_usages: Vec<Bound<'py, PyAny>> =
            usage_records(&native_field(&model, "macro_usages")?)?;
        let declaration_usages: Vec<Bound<'py, PyAny>> =
            usage_records(&native_field(&model, "declaration_usages")?)?;
        if digests {
            sqlbuild_python_base::compile_context::main::image_facts::record_model_usages::record_model_usages(
                &model,
                (&macro_usages, &declaration_usages),
            )?;
        }
        usages.extend(macro_usages);
        usages.extend(declaration_usages);
    }
    for group in COMPILE_USAGE_GROUPS {
        for item in native_field(inputs, group)?.try_iter()? {
            usages.extend(usage_records(&native_field(&item?, "declaration_usages")?)?);
        }
    }
    let enforce_placement: bool = native_field(
        &native_field(&native_field(inputs, "project_config")?, "scopes")?,
        "enforce_placement",
    )?
    .extract()?;
    Ok(scope_placement_validated_index(
        &native_field(inputs, "scope_index")?,
        enforce_placement,
        Some(PyTuple::new(py, usages)?),
    )?
    .into_bound(py))
}
