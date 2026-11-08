//! Native output-type recovery for failed models, revalidating planned models in Python.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyList, PyString, PyTuple};
use sqlbuild_analysis::analysis_route::main::analysis_route::analysis_route;
use sqlbuild_analysis::analysis_route::main::native_revalidation::native_revalidation;
use sqlbuild_analysis::analysis_route::models::AnalysisRoute;
use sqlbuild_analysis::analysis_route::types::NativeDialect;
use sqlbuild_analysis::semantic_checks::main::finish_type_recovery::finish_type_recovery;
use sqlbuild_analysis::semantic_checks::main::plan_type_recovery::plan_type_recovery;
use sqlbuild_analysis::semantic_checks::models::{
    DiagnosticOwner, LineageOutput, ModelBinding, RawBinding, RecoveryModel, TypeRecoveryRequest,
};
use sqlbuild_analysis::semantic_checks::types::RevisedBinding;
use sqlbuild_analysis::semantic_validation::models as validation;

use crate::bindings::_helpers::analysis::binding_catalog::schema_validations;
use crate::bindings::_helpers::analysis_session::session::NativeModelAnalysisSession;
use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::compile_driver::stage_work::count_stage;
use crate::bindings::_helpers::compiled_views::compiled_model_view::LineageInput;
use crate::bindings::_helpers::compiled_views::model_reads::{inferred_names, lineage_inputs};
use crate::bindings::_helpers::compiled_views::records::with_type_recovery;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::fields::mapping_values;
use crate::bindings::_helpers::semantic_checks::constants::SCHEMA_REVALIDATIONS;
use crate::bindings::_helpers::semantic_checks::failures::{internal_error, semantic_error};
use crate::bindings::_helpers::semantic_checks::inputs::revalidation_request;
use crate::bindings::_helpers::semantic_checks::session_facts::{
    column_names, session_model_facts,
};
use crate::bindings::_helpers::semantic_checks::stage_objects::StageClasses;
use crate::bindings::_helpers::semantic_shapes::shapes::project_shapes;
use crate::bindings::types::CompilerDetach;

const UNCHANGED: &str = "unchanged";
const UNKNOWN_RECOVERED_TYPE: &str = "UNKNOWN";

/// The analysis state every semantic completion step shares.
pub(crate) struct StageShared<'a> {
    pub(crate) catalog: &'a validation::ProjectCatalog,
    pub(crate) session: Option<&'a NativeModelAnalysisSession>,
    /// Models whose output names and lineage the finished session supplies.
    pub(crate) session_models: HashSet<String>,
}

/// The project with recovered output types; unchanged when no model has a blocking binding.
pub(crate) fn recovered_types<'py>(
    classes: &StageClasses<'py>,
    project: &Bound<'py, PyAny>,
    binding_results: &Bound<'py, PyAny>,
    shared: &StageShared<'_>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let models: Vec<Bound<'py, PyAny>> = items(&native_field(project, "models")?)?;
    if !blocking_binding_found(&models)? {
        return Ok(project.clone());
    }
    let ids: Bound<'py, PyDict> = diagnostic_ids(project, &models)?;
    let raw_ids: Bound<'py, PyDict> = PyDict::new(py);
    for rows in mapping_values(binding_results)?.try_iter()? {
        for row in rows?.try_iter()? {
            let row: Bound<'py, PyAny> = row?;
            if !raw_ids.contains(&row)? {
                raw_ids.set_item(&row, raw_ids.len())?;
            }
        }
    }
    let diagnostics: Vec<Bound<'py, PyAny>> = items(&native_field(project, "diagnostics")?)?;
    let mut owners: Vec<DiagnosticOwner> = Vec::with_capacity(diagnostics.len());
    for item in &diagnostics {
        owners.push(DiagnosticOwner {
            id: id_of(&ids, item)?,
            code: native_field(item, "code")?.extract()?,
            is_model: native_field(item, "resource_type")?.eq(&classes.model_resource)?,
            resource_name: native_field(item, "resource_name")?.extract()?,
        });
    }
    let mut recovery_models: Vec<RecoveryModel> = Vec::with_capacity(models.len());
    for model in &models {
        recovery_models.push(recovery_model(
            model,
            binding_results,
            (&ids, &raw_ids),
            shared,
        )?);
    }
    let request = TypeRecoveryRequest {
        dialect: native_field(project, "sql_analysis_dialect")?.extract()?,
        models: recovery_models,
        diagnostics: owners,
    };
    let step = py
        .compiler_detach(|| Ok(plan_type_recovery(&request, shared.catalog)))
        .map_err(compiler_error)?
        .map_err(|failure| semantic_error(py, &failure))?;
    if step.status() == UNCHANGED {
        return Ok(project.clone());
    }
    let plan = step
        .plan()
        .ok_or_else(|| internal_error("finishing a type recovery that has no plan"))?;
    let poisoned: Vec<(String, String)> = plan.poisoned_outputs();
    let models_to_revalidate: Vec<usize> = plan.revalidated_models();
    let revised: Vec<Vec<RevisedBinding>> = match analysis_route(request.dialect.as_deref()) {
        AnalysisRoute::Native(dialect) => {
            natively_revalidated(dialect, project, (&models_to_revalidate, &poisoned), shared)?
        }
        AnalysisRoute::Legacy | AnalysisRoute::Unavailable => {
            revalidated(classes, project, &models_to_revalidate, &poisoned)?
        }
    };
    let outcome =
        finish_type_recovery(&request, plan, &revised).map_err(|reason| internal_error(&reason))?;
    let mut kept: Vec<Bound<'py, PyAny>> = Vec::with_capacity(outcome.kept.len());
    for (index, note) in outcome.kept {
        let original: &Bound<'py, PyAny> = &diagnostics[index];
        let mut notes: Vec<Bound<'py, PyAny>> = items(&native_field(original, "notes")?)?;
        if let Some(note) = note {
            notes.push(PyString::new(py, &note).into_any());
        }
        kept.push(classes.replaced(original, &[("notes", &PyTuple::new(py, notes)?.into_any())])?);
    }
    let poisoned_outputs: Bound<'py, PyFrozenSet> = PyFrozenSet::new(py, &poisoned)?;
    let mut recovered: Vec<Bound<'py, PyAny>> = Vec::with_capacity(models.len());
    for (model, positions) in models.iter().zip(outcome.model_bindings) {
        let selected: Vec<Bound<'py, PyAny>> = positions
            .into_iter()
            .map(|position| kept[position].clone())
            .collect();
        recovered.push(with_type_recovery(
            model,
            &PyTuple::new(py, selected)?.into_any(),
            &poisoned_outputs,
        )?);
    }
    classes.project_with(
        project,
        &[
            ("models", &PyTuple::new(py, recovered)?.into_any()),
            ("diagnostics", &PyTuple::new(py, kept)?.into_any()),
        ],
    )
}

/// Every item of a Python iterable.
pub(crate) fn items<'py>(value: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    value.try_iter()?.collect()
}

/// One id per distinct diagnostic value, project diagnostics first, as Python's dicts see them.
pub(crate) fn diagnostic_ids<'py>(
    project: &Bound<'py, PyAny>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyDict>> {
    let ids: Bound<'py, PyDict> = PyDict::new(project.py());
    for item in native_field(project, "diagnostics")?.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        if !ids.contains(&item)? {
            ids.set_item(&item, ids.len())?;
        }
    }
    for model in models {
        for item in native_field(model, "binding_diagnostics")?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            if !ids.contains(&item)? {
                ids.set_item(&item, ids.len())?;
            }
        }
    }
    Ok(ids)
}

pub(crate) fn id_of(ids: &Bound<'_, PyDict>, item: &Bound<'_, PyAny>) -> PyResult<usize> {
    ids.as_any().get_item(item)?.extract()
}

pub(crate) fn lineage_outputs(lineage: Vec<LineageInput>) -> Vec<LineageOutput> {
    lineage
        .into_iter()
        .map(|(output_column, upstream)| LineageOutput {
            output_column,
            upstream,
        })
        .collect()
}

fn blocking_binding_found(models: &[Bound<'_, PyAny>]) -> PyResult<bool> {
    for model in models {
        for item in native_field(model, "binding_diagnostics")?.try_iter()? {
            if native_field(&item?, "is_error")?.is_truthy()? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn recovery_model(
    model: &Bound<'_, PyAny>,
    binding_results: &Bound<'_, PyAny>,
    (ids, raw_ids): (&Bound<'_, PyDict>, &Bound<'_, PyDict>),
    shared: &StageShared<'_>,
) -> PyResult<RecoveryModel> {
    let name: String = native_field(model, "name")?.extract()?;
    let from_session: bool = shared.session_models.contains(&name);
    let bindings: Vec<Bound<'_, PyAny>> = items(&native_field(model, "binding_diagnostics")?)?;
    let mut model_bindings: Vec<ModelBinding> = Vec::with_capacity(bindings.len());
    for item in &bindings {
        model_bindings.push(ModelBinding {
            id: id_of(ids, item)?,
            code: native_field(item, "code")?.extract()?,
            message: native_field(item, "message")?.extract()?,
            is_error: native_field(item, "is_error")?.extract()?,
        });
    }
    let mut raw_bindings: Vec<RawBinding> = Vec::new();
    if !bindings.is_empty() {
        let raws: Bound<'_, PyAny> =
            mapping_get_or(binding_results, &name, PyList::empty(model.py()))?;
        for raw in raws.try_iter()? {
            let raw: Bound<'_, PyAny> = raw?;
            raw_bindings.push(RawBinding {
                id: id_of(raw_ids, &raw)?,
                code: raw.get_item(0)?.extract()?,
                message: raw.get_item(1)?.extract()?,
                start: raw.get_item(4)?.extract()?,
                end: raw.get_item(5)?.extract()?,
            });
        }
    }
    let (inferred_columns, lineage) = if from_session {
        let facts = session_model_facts(shared.session, &name).map_err(compiler_error)?;
        (column_names(facts), facts.lineage.clone())
    } else {
        (inferred_names(model)?, lineage_inputs(model)?)
    };
    let mut references: Vec<String> = Vec::new();
    for reference in native_field(model, "references")?.try_iter()? {
        references.push(native_field(&reference?, "ref_name")?.extract()?);
    }
    Ok(RecoveryModel {
        name,
        query_sql: native_field(model, "query_sql")?.extract()?,
        inferred_columns,
        references,
        lineage: lineage_outputs(lineage),
        bindings: model_bindings,
        raw_bindings,
    })
}

/// Each planned model revalidated by the dialect's native analysis, never the Polyglot catalog.
fn natively_revalidated(
    dialect: &'static dyn NativeDialect,
    project: &Bound<'_, PyAny>,
    (models, poisoned): (&[usize], &[(String, String)]),
    shared: &StageShared<'_>,
) -> PyResult<Vec<Vec<RevisedBinding>>> {
    if models.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = project.py();
    let session: &NativeModelAnalysisSession = shared
        .session
        .ok_or_else(|| internal_error("revalidating a native route without its session"))?;
    let finished = session
        .finished_session()
        .ok_or_else(|| internal_error("revalidating before the session finished"))?;
    let project_models: Bound<'_, PyAny> = native_field(project, "models")?;
    let mut names: Vec<String> = Vec::with_capacity(models.len());
    for index in models {
        names.push(native_field(&project_models.get_item(*index)?, "name")?.extract()?);
    }
    count_stage(py, SCHEMA_REVALIDATIONS, names.len() as u64)?;
    py.compiler_detach(|| {
        Ok(native_revalidation(
            dialect,
            (finished, session.outcomes()),
            &names,
            poisoned,
        ))
    })
    .map_err(compiler_error)?
    .map_err(|reason| internal_error(&reason))
}

/// Python's revalidation of each planned model with unknown types for poisoned outputs.
fn revalidated<'py>(
    classes: &StageClasses<'py>,
    project: &Bound<'py, PyAny>,
    models: &[usize],
    poisoned: &[(String, String)],
) -> PyResult<Vec<Vec<RevisedBinding>>> {
    if models.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'py> = classes.py();
    let shapes: Bound<'py, PyDict> = project_shapes(project, None)?;
    let unknown: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(&shapes)?.try_iter()? {
        let (name, columns): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let copied: Bound<'py, PyDict> = PyDict::new(py);
        copied.update(columns.downcast::<PyDict>()?.as_mapping())?;
        unknown.set_item(name, copied)?;
    }
    for (name, column) in poisoned {
        if let Some(columns) = unknown.get_item(name)?
            && columns.contains(column)?
        {
            columns.set_item(column, UNKNOWN_RECOVERED_TYPE)?;
        }
    }
    let project_models: Bound<'py, PyAny> = native_field(project, "models")?;
    let mut revised: Vec<Vec<RevisedBinding>> = Vec::with_capacity(models.len());
    for index in models {
        let request: Bound<'py, PyAny> =
            revalidation_request(project, &project_models.get_item(*index)?, &unknown)?;
        count_stage(py, SCHEMA_REVALIDATIONS, 1)?;
        let result: Bound<'py, PyAny> =
            schema_validations(py, &PyTuple::new(py, [request])?.into_any())?.get_item(0)?;
        let mut rows: Vec<RevisedBinding> = Vec::new();
        for item in native_field(&result, "diagnostics")?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            rows.push((
                native_field(&item, "code")?.extract()?,
                native_field(&item, "start")?.extract()?,
                native_field(&item, "end")?.extract()?,
            ));
        }
        revised.push(rows);
    }
    Ok(revised)
}
