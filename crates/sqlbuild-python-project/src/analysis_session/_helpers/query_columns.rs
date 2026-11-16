//! Python's per-query column analysis outside the session, run natively.

use crate::analysis_session::main::query_columns_error::query_columns_error;
use crate::analysis_session::types::ColumnsRow;
use pyo3::prelude::{Py, PyAny, PyRef, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::analysis_session::main::projection_facts::projection_facts;
use sqlbuild_analysis::assembly::analysis_session::main::query_columns::query_columns;
use sqlbuild_analysis::assembly::analysis_session::models::{
    ColumnFact, ProjectionFacts, QueryColumns, QueryColumnsInput, QueryColumnsMode,
    QueryColumnsRequest,
};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_python_base::positional_record;

use crate::analysis::models::ProjectCatalog;
use crate::analysis_session::_helpers::nullability_rules::{
    RuleFailure, nullability_callback, raised,
};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

pub(crate) const QUERY_COLUMNS_CONTEXT: &str = "native query column analysis";
const PROJECTION_FACTS_CONTEXT: &str = "native fixture projection facts";

positional_record! {
    extract;
    /// `(sql, placeholders, analysis names, lineage references, recover CTE facts, mode)`.
    pub(crate) struct QueryRow {
        pub(crate) sql: String,
        pub(crate) placeholders: Pairs,
        pub(crate) analysis_names: Vec<String>,
        pub(crate) lineage_references: Vec<(String, String, String)>,
        pub(crate) recover_cte_facts: bool,
        pub(crate) mode: String,
    }
}

positional_record! {
    extract;
    /// `(dialect, return types, rules, types, nullability, queries, normalization dialect)`.
    pub(crate) struct QueryRequestRow {
        pub(crate) dialect: String,
        pub(crate) function_return_types: Pairs,
        pub(crate) nullability_rules: Pairs,
        pub(crate) types: Shapes,
        pub(crate) nullability: Shapes,
        pub(crate) queries: Vec<QueryRow>,
        pub(crate) normalization_dialect: String,
    }
}
/// `([(name, type)], NULL literal names, quoted names)`.
type ProjectionFactsRow = (Vec<(String, String)>, Vec<String>, Vec<String>);

/// Each query's columns, as Python's analysis outside the session inferred them.
#[pyfunction]
#[pyo3(signature = (catalog, request, adapter_rules=None))]
pub(crate) fn infer_query_columns(
    py: Python<'_>,
    catalog: Option<PyRef<'_, ProjectCatalog>>,
    request: QueryRequestRow,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<Vec<ColumnsRow>> {
    let QueryRequestRow {
        dialect,
        function_return_types,
        nullability_rules,
        types,
        nullability,
        queries,
        normalization_dialect,
    } = request;
    let queries: Vec<QueryColumnsInput> = queries
        .into_iter()
        .map(query_input)
        .collect::<Result<_, String>>()
        .map_err(|reason| compiler_error(native_failure(QUERY_COLUMNS_CONTEXT, &reason)))?;
    let rule_failure: RuleFailure = RuleFailure::default();
    let request = QueryColumnsRequest {
        dialect,
        normalization_dialect,
        function_return_types,
        nullability_rules: Some(nullability_rules),
        nullability_callback: adapter_rules.map(|(rules, nullability)| {
            nullability_callback(rules, nullability, rule_failure.clone())
        }),
        types,
        nullability,
        queries,
    };
    let catalog = catalog.as_ref().map(|catalog| &catalog.inner);
    let columns = py.compiler_detach(|| Ok(query_columns(catalog, &request)));
    if let Some(error) = raised(&rule_failure) {
        return Err(error);
    }
    Ok(columns
        .map_err(|reason| compiler_error(native_failure(QUERY_COLUMNS_CONTEXT, &reason)))?
        .map_err(query_columns_error)?
        .into_iter()
        .map(columns_row)
        .collect())
}

fn query_input(row: QueryRow) -> Result<QueryColumnsInput, String> {
    let QueryRow {
        sql,
        placeholders,
        analysis_names,
        lineage_references,
        recover_cte_facts,
        mode,
    } = row;
    let mode: QueryColumnsMode = match mode.as_str() {
        "batch" => QueryColumnsMode::Batch,
        "reanalysis" => QueryColumnsMode::Reanalysis,
        "parse" => QueryColumnsMode::Parse,
        "legacy" => QueryColumnsMode::Legacy,
        _ => return Err("an unknown query column analysis mode".to_owned()),
    };
    Ok(QueryColumnsInput {
        sql,
        placeholders,
        analysis_names,
        lineage_references,
        recover_cte_facts,
        mode,
    })
}

fn columns_row(columns: QueryColumns) -> ColumnsRow {
    (
        columns.succeeded,
        columns
            .columns
            .map(|columns| columns.into_iter().map(column_row).collect()),
        columns.has_star,
    )
}

/// Python's `_simple_projection_facts`: `(types, NULL literal names, quoted names)`, casefolded.
#[pyfunction]
pub(crate) fn fixture_projection_facts(
    py: Python<'_>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<ProjectionFactsRow> {
    let facts: ProjectionFacts = py
        .compiler_detach(|| projection_facts(sql, dialect))
        .map_err(|reason| compiler_error(native_failure(PROJECTION_FACTS_CONTEXT, &reason)))?;
    Ok((facts.types, facts.null_literal_names, facts.quoted_names))
}

fn column_row(column: ColumnFact) -> (String, Option<String>, String) {
    (column.name, column.data_type, column.nullability)
}
