//! One model rename, model move, or column rename planned from compiler facts.

use std::path::Path;

use crate::refactoring::_helpers::edits::text_edits::build_plan;
use crate::refactoring::_helpers::planning::column_planning::{
    column_rename_context, column_rename_parts,
};
use crate::refactoring::_helpers::planning::job_selectors::job_selector_parts;
use crate::refactoring::_helpers::planning::model_planning::{model_parts, model_target};
use crate::refactoring::_helpers::planning::python_imports::python_import_parts;
use crate::refactoring::_helpers::scanning::scan_context::ScanContext;
use crate::refactoring::constants::{COLUMN_MANUAL_HELP, MODEL_MANUAL_HELP};
use crate::refactoring::errors::RefactorError;
use crate::refactoring::models::{RefactorFacts, RefactorOperation, RefactorPlan, RefactorRequest};

use crate::refactoring::types::{DeclarationMoveHost, UnitTestScopeHost};

/// Plan one refactoring without touching files, asking the hosts about declaration scopes.
pub(crate) fn plan_single(
    facts: &RefactorFacts,
    request: &RefactorRequest,
    host: &DeclarationMoveHost<'_>,
    test_host: &UnitTestScopeHost<'_>,
) -> Result<RefactorPlan, RefactorError> {
    let context = ScanContext::for_facts(facts);
    if request.operation == RefactorOperation::RenameColumn {
        let column = column_rename_context(facts, request, &context)?;
        let parts = column_rename_parts(&column)?;
        let renamed: Vec<(String, String, String)> = parts
            .cascaded
            .iter()
            .map(|model| (model.clone(), column.old.clone(), column.new.clone()))
            .collect();
        return Ok(build_plan(
            request.clone(),
            parts,
            &[],
            (renamed, Some(COLUMN_MANUAL_HELP)),
        ));
    }
    let target = model_target(facts, request)?;
    let mut parts = model_parts(facts, &target, &context, (host, test_host))?;
    let mut moves = parts.moves.clone();
    let destination = target.destination();
    if destination != target.source_path {
        moves.push((target.source_path.clone(), destination.clone()));
    }
    let imports = python_import_parts(Path::new(&facts.project_dir), &moves)?;
    parts.edits.extend(imports.edits);
    parts.blocking.extend(imports.blocking);
    let selectors = job_selector_parts(
        Path::new(&facts.project_dir),
        &facts.models,
        &[(target.model, &target.request.new_name, &destination)],
    )?;
    parts.edits.extend(selectors.edits);
    parts.manual.extend(selectors.manual);
    parts.notes.extend(selectors.notes);
    parts.blocking.extend(selectors.blocking);
    Ok(build_plan(
        target.request,
        parts,
        &moves,
        (Vec::new(), Some(MODEL_MANUAL_HELP)),
    ))
}
