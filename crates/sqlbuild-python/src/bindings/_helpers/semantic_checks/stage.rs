//! The native semantic completion stage: type recovery, metadata checks, then completion.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyRef, PyResult};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::analysis_session::session::NativeModelAnalysisSession;
use crate::bindings::_helpers::compiled_views::model_reads::output_proven;
use crate::bindings::_helpers::rules::early_builtin_rules::start_early_rules;
use crate::bindings::_helpers::semantic_checks::completion::completed_diagnostics;
use crate::bindings::_helpers::semantic_checks::metadata::metadata_diagnostics;
use crate::bindings::_helpers::semantic_checks::stage_objects::StageClasses;
use crate::bindings::_helpers::semantic_checks::type_recovery::{
    StageShared, items, recovered_types,
};
use crate::bindings::models::ProjectCatalog;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const NO_CATALOG_MESSAGE: &str =
    "NativeCompilerError: native semantic completion: the project has no analysis catalog";

/// The completed project from keyword arguments; an internal failure raises `NativeCompilerError`.
#[pyfunction]
#[pyo3(signature = (**request))]
fn complete_semantic_project<'py>(
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

/// The semantic completion stage's inputs.
pub(crate) struct SemanticCompletion<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) profile: Bound<'py, PyAny>,
    pub(crate) binding_results: Bound<'py, PyAny>,
    pub(crate) resource_sql_analysis: bool,
    pub(crate) session: Option<Bound<'py, PyAny>>,
}

/// The completed project; an internal failure raises `NativeCompilerError`.
pub(crate) fn completed_semantic_project<'py>(
    stage: SemanticCompletion<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let SemanticCompletion {
        project,
        profile,
        binding_results,
        resource_sql_analysis,
        session,
    } = stage;
    let session: Option<PyRef<'py, NativeModelAnalysisSession>> = match session {
        Some(session) if !session.is_none() => Some(session.extract()?),
        _ => None,
    };
    let catalog: PyRef<'py, ProjectCatalog> = project_catalog(&project)?;
    let classes = StageClasses::new(project.py())?;
    let shared = StageShared {
        catalog: &catalog.inner,
        session: session.as_deref(),
        session_models: session_fact_models(&project, session.as_deref())?,
    };
    let recovered: Bound<'py, PyAny> =
        recovered_types(&classes, &project, &binding_results, &shared)?;
    start_early_rules(&recovered)?;
    let metadata: Vec<Bound<'py, PyAny>> = metadata_diagnostics(
        &classes,
        (&recovered, &profile),
        resource_sql_analysis,
        &catalog.inner,
    )?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = items(&native_field(&recovered, "diagnostics")?)?;
    diagnostics.extend(metadata);
    let combined: Bound<'py, PyAny> = classes.project_with(
        &recovered,
        &[(
            "diagnostics",
            &PyTuple::new(project.py(), diagnostics)?.into_any(),
        )],
    )?;
    completed_diagnostics(&classes, &combined, &shared)
}

/// The metadata and resource SQL diagnostics alone, as the stage appends them.
#[pyfunction]
#[pyo3(signature = (*, project, profile, resource_sql_analysis))]
fn semantic_metadata_diagnostics<'py>(
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

fn project_catalog<'py>(project: &Bound<'py, PyAny>) -> PyResult<PyRef<'py, ProjectCatalog>> {
    let binding_catalog: Bound<'py, PyAny> = native_field(project, "binding_catalog")?;
    if binding_catalog.is_none() {
        return Err(native_compiler_error(NO_CATALOG_MESSAGE));
    }
    native_field(&binding_catalog, "native")?.extract()
}

/// Models whose output names and lineage are the finished session's, so need no payload.
fn session_fact_models(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(complete_semantic_project, module)?)?;
    module.add_function(wrap_pyfunction!(semantic_metadata_diagnostics, module)?)
}
