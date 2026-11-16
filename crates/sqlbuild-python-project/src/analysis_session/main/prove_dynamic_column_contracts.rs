//! Each model's dynamic pivot proof outside any session.

use crate::analysis_session::_helpers::session;
use crate::analysis_session::_helpers::session::ContractRow;
use crate::analysis_session::models::PivotRequestRow;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Each model's dynamic pivot proof outside any session.
#[inline]
pub fn prove_dynamic_column_contracts(
    py: Python<'_>,
    request: PivotRequestRow,
) -> PyResult<Vec<ContractRow>> {
    session::prove_dynamic_column_contracts(py, request)
}
