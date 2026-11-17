//! The completed project; an internal failure raises `NativeCompilerError`.

use crate::rules::main::checking::start_early_rules::start_early_rules;
use crate::semantic_checks::_helpers::recovery::completion::completed_diagnostics;
use crate::semantic_checks::_helpers::recovery::type_recovery::StageShared;
use crate::semantic_checks::_helpers::recovery::type_recovery::items;
use crate::semantic_checks::_helpers::recovery::type_recovery::recovered_types;
use crate::semantic_checks::_helpers::session::metadata::metadata_diagnostics;
use crate::semantic_checks::_helpers::session::stage::project_catalog;
use crate::semantic_checks::_helpers::session::stage::session_fact_models;
use crate::semantic_checks::_helpers::session::stage_objects::StageClasses;
use crate::semantic_checks::models::SemanticCompletion;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::analysis::models::ProjectCatalog;
use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;

/// The completed project; an internal failure raises `NativeCompilerError`.
pub fn completed_semantic_project<'py>(
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
