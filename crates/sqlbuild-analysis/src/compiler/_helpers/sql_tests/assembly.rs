//! Python's `_assemble_compiled_sql_test` facts: name, targets, scope, case identity and P013s.

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::compiler::_helpers::sql_tests::case_identity::{FingerprintFailure, case_fingerprint};
use crate::compiler::_helpers::sql_tests::macro_call_names::macro_call_names;
use crate::compiler::_helpers::sql_tests::mock_reads::{
    MockReadsRequest, mock_reading_helper_diagnostics,
};
use crate::compiler::models::{
    AssembledSqlTestFacts, SqlTestAssemblyBatch, SqlTestAssemblyFailure, SqlTestAssemblyModel,
    SqlTestAssemblyOutcome, SqlTestAssemblyPayload, SqlTestAssemblyTest, SqlTestHelperDiagnostic,
};

const MODEL_RESOURCE: &str = "model";
const TABLE_FUNCTION_RESOURCE: &str = "table_fn";
const MACRO_MODE: &str = "macro";
const UDF_MODE: &str = "udf";
const UDF_REFERENCE_KIND: &str = "udf";

/// Each test's facts in request order, or why its assembly raises.
pub(crate) fn assemble_batch(batch: &SqlTestAssemblyBatch) -> Vec<SqlTestAssemblyOutcome> {
    let tests_macros = batch.tests.iter().any(|test| {
        matches!(&test.payload, SqlTestAssemblyPayload::Direct { mode, .. } if mode == MACRO_MODE)
    });
    let macro_names: Vec<MacroNames> = batch
        .models
        .iter()
        .map(|model| match &model.unscanned_macro_source {
            Some(sql) if tests_macros => macro_call_names(sql, &batch.lexical_syntax),
            Some(_) => Ok(Vec::new()),
            None => Ok(model.macro_deps.clone()),
        })
        .collect();
    let project = Project {
        models: &batch.models,
        macro_names: &macro_names,
        syntax: &batch.lexical_syntax,
    };
    batch
        .tests
        .par_iter()
        .map(|test| match assemble(test, &project) {
            Ok(outcome) => outcome,
            Err(failure) => SqlTestAssemblyOutcome::Failed(failure),
        })
        .collect()
}

/// A model's macro dependencies, or Python's scan of its pre-macro SQL when none were recorded.
type MacroNames = Result<Vec<String>, String>;

struct Project<'a> {
    models: &'a [SqlTestAssemblyModel],
    macro_names: &'a [MacroNames],
    syntax: &'a LexicalSyntax,
}

fn assemble(
    test: &SqlTestAssemblyTest,
    project: &Project<'_>,
) -> Result<SqlTestAssemblyOutcome, SqlTestAssemblyFailure> {
    let models = project.models;
    let syntax = project.syntax;
    let mut target_model_names: Vec<String> = Vec::new();
    let mut tested_resources: Vec<(String, String)> = Vec::new();
    let mut diagnostics: Vec<SqlTestHelperDiagnostic> = Vec::new();
    let scope_deps: Vec<(&'static str, String)> = match &test.payload {
        SqlTestAssemblyPayload::Direct {
            mode,
            tested_resource_names,
        } => {
            tested_resources = tested_resource_names
                .iter()
                .map(|name| (mode.clone(), name.clone()))
                .collect();
            direct_scope_deps(mode, tested_resource_names, project)?
        }
        SqlTestAssemblyPayload::Model(payload) => {
            for name in payload
                .expected_model_names
                .iter()
                .chain(&payload.assertion_target_model_names)
                .chain(&payload.reference_target_model_names)
            {
                if !target_model_names.contains(name) {
                    target_model_names.push(name.clone());
                }
            }
            diagnostics = mock_reading_helper_diagnostics(&MockReadsRequest {
                test,
                payload,
                models,
                target_model_names: &target_model_names,
                syntax,
            })?;
            target_model_names
                .iter()
                .map(|name| (MODEL_RESOURCE, name.clone()))
                .collect()
        }
    };
    let mut overflow = false;
    let case_fingerprint = match &test.case_name {
        Some(case_name) => {
            match case_fingerprint(test, case_name, &scope_deps, &tested_resources) {
                Ok(fingerprint) => Some(fingerprint),
                Err(FingerprintFailure::DecimalOverflow) => {
                    overflow = true;
                    None
                }
                Err(FingerprintFailure::Internal(reason)) => {
                    return Err(SqlTestAssemblyFailure::Internal(reason));
                }
            }
        }
        None => None,
    };
    let facts = AssembledSqlTestFacts {
        name: test_name(test),
        scope_deps,
        target_model_names,
        case_fingerprint,
        diagnostic_resource_name: non_empty(&test.block_name)
            .unwrap_or(&test.relative_stem)
            .to_owned(),
        diagnostics,
    };
    Ok(if overflow {
        SqlTestAssemblyOutcome::FingerprintOverflow(facts)
    } else {
        SqlTestAssemblyOutcome::Assembled(facts)
    })
}

fn non_empty(name: &Option<String>) -> Option<&str> {
    name.as_deref().filter(|name| !name.is_empty())
}

/// Python's `_resolve_test_name`.
fn test_name(test: &SqlTestAssemblyTest) -> String {
    let parent = non_empty(&test.block_name).unwrap_or(&test.file_stem);
    match &test.case_name {
        Some(case_name) => format!("{parent} [{case_name}]"),
        None => parent.to_owned(),
    }
}

/// Python's macro, UDF and table-function scope dependencies of a direct-logic test.
fn direct_scope_deps(
    mode: &str,
    tested_names: &[String],
    project: &Project<'_>,
) -> Result<Vec<(&'static str, String)>, SqlTestAssemblyFailure> {
    let mut scope_deps: Vec<(&'static str, String)> = Vec::new();
    match mode {
        MACRO_MODE => {
            for (model, names) in project.models.iter().zip(project.macro_names) {
                let names = names
                    .as_ref()
                    .map_err(|message| SqlTestAssemblyFailure::Input(message.clone()))?;
                if names.iter().any(|name| tested_names.contains(name)) {
                    scope_deps.push((MODEL_RESOURCE, model.name.clone()));
                }
            }
        }
        UDF_MODE => {
            for model in project.models {
                if model.references.iter().any(|reference| {
                    reference.kind == UDF_REFERENCE_KIND && tested_names.contains(&reference.name)
                }) {
                    scope_deps.push((MODEL_RESOURCE, model.name.clone()));
                }
            }
        }
        _ => scope_deps.extend(
            tested_names
                .iter()
                .map(|name| (TABLE_FUNCTION_RESOURCE, name.clone())),
        ),
    }
    Ok(scope_deps)
}
