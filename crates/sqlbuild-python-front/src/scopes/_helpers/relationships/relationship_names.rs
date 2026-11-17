//! Expected-model relationship names and top-level CTEs of SQL tests and scenarios, scanned natively.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_scopes::relationship_names::main::expected_model_names::expected_model_names;
use sqlbuild_scopes::relationship_names::main::top_level_ctes::scan_top_level_ctes;
use sqlbuild_scopes::relationship_names::models::{
    ExpectedNames, RelationshipSource, TopLevelCtes,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::sqltext::models::LexicalSyntaxInput;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// One scan's error, or its values.
type Scanned<T> = (Option<String>, Vec<T>);

fn source(scenario: bool) -> RelationshipSource {
    if scenario {
        RelationshipSource::Scenario
    } else {
        RelationshipSource::Test
    }
}

/// Each `(sql, file label)` body's expected models or the scan's error.
#[pyfunction]
pub(crate) fn scope_expected_model_names(
    py: Python<'_>,
    texts: Vec<(String, String)>,
    scenario: bool,
    syntax: LexicalSyntaxInput,
) -> PyResult<Vec<Scanned<String>>> {
    let syntax: LexicalSyntax = syntax.into();
    let outcomes: Vec<ExpectedNames> = py
        .compiler_detach(|| Ok(expected_model_names(&texts, source(scenario), &syntax)))
        .map_err(compiler_error)?;
    Ok(outcomes
        .into_iter()
        .map(|outcome| match outcome {
            ExpectedNames::Scanned(names) => (None, names),
            ExpectedNames::Failed(message) => (Some(message), Vec::new()),
        })
        .collect())
}

/// Each `(sql, file label)` test body's top-level CTEs or the scan's error.
#[pyfunction]
pub(crate) fn scope_test_ctes(
    py: Python<'_>,
    texts: Vec<(String, String)>,
    syntax: LexicalSyntaxInput,
) -> PyResult<Vec<Scanned<(String, String)>>> {
    let syntax: LexicalSyntax = syntax.into();
    let outcomes: Vec<TopLevelCtes> = py
        .compiler_detach(|| {
            Ok(texts
                .iter()
                .map(|(sql, file)| {
                    scan_top_level_ctes(sql, file, RelationshipSource::Test, &syntax)
                })
                .collect())
        })
        .map_err(compiler_error)?;
    Ok(outcomes
        .into_iter()
        .map(|outcome| match outcome {
            TopLevelCtes::Scanned(ctes) => (None, ctes),
            TopLevelCtes::Failed(message) => (Some(message), Vec::new()),
        })
        .collect())
}
