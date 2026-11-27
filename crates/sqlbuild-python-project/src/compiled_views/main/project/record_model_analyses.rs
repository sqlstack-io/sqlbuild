//! Transfer each assembled model's typed analysis to its retained facts; return how many.

use crate::compiled_project::models::{AnalysisPy, NativeCompiledProject, ProofPy};
use crate::compiled_views::_helpers::project::records::family_types;
use crate::compiled_views::models::NativeCompiledModel;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Transfer each assembled model's typed analysis to its retained facts; return how many.
#[pyfunction]
#[pyo3(signature = (*, project, models))]
pub fn record_model_analyses<'py>(
    project: &Bound<'py, NativeCompiledProject>,
    models: &Bound<'py, PyAny>,
) -> PyResult<usize> {
    let py: Python<'py> = project.py();
    let models: Vec<Bound<'py, PyAny>> = models.try_iter()?.collect::<PyResult<_>>()?;
    let views: Vec<PyRef<'py, NativeCompiledModel>> = models
        .iter()
        .filter(|model| model.is_instance_of::<NativeCompiledModel>())
        .map(|model| {
            model
                .extract::<PyRef<'py, NativeCompiledModel>>()
                .map_err(Into::into)
        })
        .collect::<PyResult<_>>()?;
    let mut transferred: usize = NativeCompiledProject::record_view_analyses(project, views)?;
    let nullable: Bound<'py, PyAny> = enum_member(py, RecordId::InferredNullability, "NULLABLE")?;
    for model in &models {
        if model.is_instance_of::<NativeCompiledModel>() {
            continue;
        }
        let inferred: Bound<'py, PyAny> = native_field(model, "inferred_rows")?;
        let rows: Vec<(String, Option<String>, Bound<'py, PyAny>)> = if inferred.is_none() {
            Vec::new()
        } else {
            inferred.extract()?
        };
        let proof: Bound<'py, PyAny> = native_field(model, "dynamic_column_contract")?;
        let proof = if proof.is_none() {
            None
        } else {
            let families: Bound<'py, PyAny> = native_field(&proof, "families")?;
            Some(ProofPy {
                output_proven: native_field(&proof, "output_proven")?.extract()?,
                bare_dynamic_pivot: native_field(&proof, "bare_dynamic_pivot")?.extract()?,
                families: family_types(&families, true)?,
                failure_reason: native_field(&proof, "failure_reason")?.extract()?,
                named_families: family_types(&families, false)?,
            })
        };
        let analysis = AnalysisPy {
            inferred: !inferred.is_none(),
            inferred_columns: rows
                .iter()
                .map(|(name, column_type, _)| (name.clone(), column_type.clone()))
                .collect(),
            nullable_columns: rows
                .iter()
                .map(|(_, _, nullability)| nullability.eq(&nullable))
                .collect::<PyResult<_>>()?,
            fast_lineage_has_star: native_field(model, "fast_lineage_has_star")?.is_truthy()?,
            unchecked: native_field(model, "unchecked_output_columns")?
                .try_iter()?
                .map(|column| column?.extract())
                .collect::<PyResult<_>>()?,
        };
        let name: String = native_field(model, "name")?.extract()?;
        transferred += usize::from(
            project
                .borrow_mut()
                .record_model_analysis(&name, analysis, proof),
        );
    }
    Ok(transferred)
}
