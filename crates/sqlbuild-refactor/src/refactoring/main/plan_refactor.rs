//! Plan a model rename, column rename, or one or more model moves from compiler facts.

use std::path::Path;

use crate::refactoring::_helpers::edits::text_edits::build_plan;
use crate::refactoring::_helpers::planning::batch_planning::batch_parts;
use crate::refactoring::_helpers::planning::job_selectors::job_selector_parts;
use crate::refactoring::_helpers::planning::model_planning::{ModelTarget, model_target};
use crate::refactoring::_helpers::planning::python_imports::python_import_parts;
use crate::refactoring::_helpers::planning::single_planning::plan_single;
use crate::refactoring::_helpers::scanning::scan_context::ScanContext;
use crate::refactoring::constants::MODEL_MANUAL_HELP;
use crate::refactoring::errors::RefactorError;
use crate::refactoring::models::{RefactorFacts, RefactorPlan, RefactorRequest};
use crate::refactoring::types::{DeclarationMoveHost, UnitTestScopeHost};

/// Plan `requests` together against the state after all of them, without touching files.
pub fn plan_refactor(
    facts: &RefactorFacts,
    requests: &[RefactorRequest],
    host: &DeclarationMoveHost<'_>,
    test_host: &UnitTestScopeHost<'_>,
) -> Result<RefactorPlan, RefactorError> {
    let [first, rest @ ..] = requests else {
        return Err(RefactorError::internal(
            "a refactoring needs at least one request",
        ));
    };
    if rest.is_empty() {
        return plan_single(facts, first, host, test_host);
    }
    let context = ScanContext::for_facts(facts);
    let targets: Vec<ModelTarget<'_>> = requests
        .iter()
        .map(|request| model_target(facts, request))
        .collect::<Result<_, _>>()?;
    let mut parts = batch_parts(facts, &targets, &context, (host, test_host))?;
    let moves: Vec<(String, String)> = parts.moves.clone();
    let imports = python_import_parts(Path::new(&facts.project_dir), &moves)?;
    parts.edits.extend(imports.edits);
    parts.blocking.extend(imports.blocking);
    let destinations: Vec<String> = targets.iter().map(ModelTarget::destination).collect();
    let renamed: Vec<(&_, &str, &str)> = targets
        .iter()
        .zip(&destinations)
        .map(|(target, destination)| {
            (
                target.model,
                target.request.new_name.as_str(),
                destination.as_str(),
            )
        })
        .collect();
    let selectors = job_selector_parts(Path::new(&facts.project_dir), &facts.models, &renamed)?;
    parts.edits.extend(selectors.edits);
    parts.manual.extend(selectors.manual);
    parts.notes.extend(selectors.notes);
    parts.blocking.extend(selectors.blocking);
    let batch: Vec<RefactorRequest> = targets.into_iter().map(|target| target.request).collect();
    let mut plan = build_plan(
        batch[0].clone(),
        parts,
        &moves,
        (Vec::new(), Some(MODEL_MANUAL_HELP)),
    );
    plan.batch = batch;
    Ok(plan)
}
