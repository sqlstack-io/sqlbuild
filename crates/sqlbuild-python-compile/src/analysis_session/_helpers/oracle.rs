//! Test-only hooks comparing native CTE fact recovery and set-operation search with Python's.

use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::assembly::analysis_session::main::names_set_operation::names_set_operation;
use sqlbuild_analysis::assembly::analysis_session::main::recover_cte_facts::recover_cte_facts;
use sqlbuild_analysis::assembly::analysis_session::models::{CteFactRequest, CteFacts};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_python_base::positional_record;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::types::CompilerDetach;

positional_record! {
    extract;
    /// One query's CTE fact recovery request, as the oracle tests send it.
    pub(crate) struct RecoveryRow {
        cleaned_sql: String,
        dialect: String,
        input_schemas: Shapes,
        function_return_types: Pairs,
        nullability_rules: Option<Pairs>,
        recover: bool,
        null_filter: bool,
    }
}

positional_record! {
    into;
    /// The deferral, types, nullability, sorted direct outputs and sorted non-null outputs.
    pub(crate) struct FactsRow {
        deferral: Option<String>,
        types: Pairs,
        nullability: Pairs,
        direct_outputs: Vec<String>,
        non_null_outputs: Vec<String>,
    }
}

/// The facts native enrichment recovers for one query, or the reason it defers to Python.
#[pyfunction]
pub(crate) fn _oracle_cte_fact_recovery(
    py: Python<'_>,
    request: RecoveryRow,
) -> PyResult<FactsRow> {
    let RecoveryRow {
        cleaned_sql,
        dialect,
        input_schemas,
        function_return_types,
        nullability_rules,
        recover,
        null_filter,
    } = request;
    let request = CteFactRequest {
        cleaned_sql,
        dialect,
        input_schemas,
        function_return_types,
        nullability_rules,
        recover,
        null_filter,
    };
    let facts: Result<CteFacts, String> = py
        .compiler_detach(|| Ok(recover_cte_facts(&request)))
        .map_err(value_error)?;
    Ok(match facts {
        Ok(facts) => FactsRow {
            deferral: None,
            types: facts.types,
            nullability: facts.nullability,
            direct_outputs: facts.direct_outputs,
            non_null_outputs: facts.non_null_outputs,
        },
        Err(deferral) => FactsRow {
            deferral: Some(deferral),
            types: Vec::new(),
            nullability: Vec::new(),
            direct_outputs: Vec::new(),
            non_null_outputs: Vec::new(),
        },
    })
}

/// Whether a model's SQL names a set operation, as the analysis session reads it.
#[pyfunction]
pub(crate) fn _oracle_names_set_operation(sql: &str) -> bool {
    names_set_operation(sql)
}
