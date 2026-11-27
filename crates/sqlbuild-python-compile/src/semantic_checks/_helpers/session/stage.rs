//! The native semantic completion stage: type recovery, metadata checks, then completion.

use crate::semantic_checks::main::completed_semantic_project::completed_semantic_project;
use crate::semantic_checks::models::SemanticCompletion;
use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};

use crate::semantic_checks::_helpers::session::metadata::metadata_diagnostics;
use crate::semantic_checks::_helpers::session::stage_objects::StageClasses;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::analysis::models::ProjectCatalog;
use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;
use sqlbuild_python_project::compiled_views::main::compiled_models::output_proven::output_proven;

const NO_CATALOG_MESSAGE: &str =
    "NativeCompilerError: native semantic completion: the project has no analysis catalog";

/// The completed project from keyword arguments; an internal failure raises `NativeCompilerError`.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn complete_semantic_project<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("complete_semantic_project", request)?;
    completed_semantic_project(SemanticCompletion {
        project: request.get("project")?,
        profile: request.get("profile")?,
        binding_results: request.get("binding_results")?,
        resource_sql_analysis: request.get("resource_sql_analysis")?.extract()?,
        session: request.optional("session")?,
    })
}

/// The metadata and resource SQL diagnostics alone, as the stage appends them.
#[pyfunction]
#[pyo3(signature = (*, project, profile, resource_sql_analysis))]
pub(crate) fn semantic_metadata_diagnostics<'py>(
    project: &Bound<'py, PyAny>,
    profile: &Bound<'py, PyAny>,
    resource_sql_analysis: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let catalog: PyRef<'py, ProjectCatalog> = project_catalog(project)?;
    let classes = StageClasses::new(project.py())?;
    let diagnostics: Vec<Bound<'py, PyAny>> = metadata_diagnostics(
        &classes,
        (project, profile),
        resource_sql_analysis,
        &catalog.inner,
    )?;
    PyTuple::new(project.py(), diagnostics)
}

pub(crate) fn project_catalog<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<PyRef<'py, ProjectCatalog>> {
    let binding_catalog: Bound<'py, PyAny> = native_field(project, "binding_catalog")?;
    if binding_catalog.is_none() {
        return Err(native_compiler_error(NO_CATALOG_MESSAGE));
    }
    native_field(&binding_catalog, "native")?
        .extract()
        .map_err(Into::into)
}

/// Models whose output names and lineage are the finished session's, so need no payload.
pub(crate) fn session_fact_models(
    project: &Bound<'_, PyAny>,
    session: Option<&NativeModelAnalysisSession>,
) -> PyResult<HashSet<String>> {
    let Some(session) = session else {
        return Ok(HashSet::new());
    };
    let kept: HashSet<String> = session.fact_models().into_iter().collect();
    let mut models: HashSet<String> = HashSet::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'_, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        if !kept.contains(&name) {
            continue;
        }
        if output_proven(&model)? {
            continue;
        }
        models.insert(name);
    }
    Ok(models)
}
