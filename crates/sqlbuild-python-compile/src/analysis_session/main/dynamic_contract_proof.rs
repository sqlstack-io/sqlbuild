//! A native dynamic pivot proof row as Python's proof, None when absent.

use crate::analysis_session::_helpers::outcome_objects::proof;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// A native dynamic pivot proof row as Python's proof, None when absent.
#[pyfunction]
pub fn dynamic_contract_proof<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    proof(row)
}
