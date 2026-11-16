//! Each model's dynamic pivot proof outside any session.

use crate::analysis_session::_helpers::session::ContractRow;
use crate::analysis_session::_helpers::session::contract_row;
use crate::analysis_session::_helpers::session::families_by_table;
use crate::analysis_session::_helpers::session::internal_error;
use crate::analysis_session::_helpers::session::pivot_model;
use crate::analysis_session::models::PivotRequestRow;
use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::assembly::analysis_session::main::prove_dynamic_contracts::prove_dynamic_contracts;
use sqlbuild_analysis::assembly::analysis_session::models::{
    PivotBatchRequest, PivotOutcome, PivotTables,
};
use sqlbuild_python_base::types::CompilerDetach;

/// Each model's dynamic pivot proof outside any session.
#[pyfunction]
pub fn prove_dynamic_column_contracts(
    py: Python<'_>,
    request: PivotRequestRow,
) -> PyResult<Vec<ContractRow>> {
    let PivotRequestRow {
        dialect,
        column_types,
        authoritative_types,
        column_nullability,
        families_by_table: by_table,
        models,
    } = request;
    let request = PivotBatchRequest {
        tables: PivotTables {
            dialect,
            column_types,
            authoritative_types,
            column_nullability,
            families_by_table: families_by_table(by_table),
        },
        models: models.into_iter().map(pivot_model).collect(),
    };
    let outcomes: Vec<PivotOutcome> = py
        .compiler_detach(|| prove_dynamic_contracts(&request))
        .map_err(|reason| internal_error(&reason))?;
    Ok(outcomes.into_iter().map(contract_row).collect())
}
