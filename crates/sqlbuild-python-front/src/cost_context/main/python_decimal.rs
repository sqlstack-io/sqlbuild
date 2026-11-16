//! `Decimal((0, digits, exponent))`.

use crate::cost_context::constants::DECIMAL_MODULE;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_core::cost_allocation::models::PyDecimal;
use sqlbuild_python_base::boundary::main::imported::imported;

/// `Decimal((0, digits, exponent))`.
pub fn python_decimal<'py>(py: Python<'py>, value: &PyDecimal) -> PyResult<Bound<'py, PyAny>> {
    let parts: Bound<'py, PyTuple> = PyTuple::new(
        py,
        [
            0_i64.into_bound_py_any(py)?,
            PyTuple::new(py, &value.digits)?.into_any(),
            value.exponent.into_bound_py_any(py)?,
        ],
    )?;
    imported(py, DECIMAL_MODULE, "Decimal")?.call1((parts,))
}
