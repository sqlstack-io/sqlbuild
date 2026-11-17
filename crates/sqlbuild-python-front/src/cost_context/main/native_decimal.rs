//! A finite non-negative `Decimal` as its exact digits and exponent.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::cost_allocation::models::PyDecimal;

/// A finite non-negative `Decimal` as its exact digits and exponent.
pub fn native_decimal(value: &Bound<'_, PyAny>) -> PyResult<PyDecimal> {
    let (sign, digits, exponent): (u8, Vec<u8>, Bound<'_, PyAny>) =
        value.call_method0("as_tuple")?.extract()?;
    let Ok(exponent) = exponent.extract::<i64>() else {
        return Err(PyValueError::new_err("cost decimals must be finite"));
    };
    if sign != 0 && digits.iter().any(|digit| *digit != 0) {
        return Err(PyValueError::new_err("cost decimals must not be negative"));
    }
    let leading: usize = digits.iter().take_while(|digit| **digit == 0).count();
    let digits: Vec<u8> = if leading == digits.len() {
        vec![0]
    } else {
        digits[leading..].to_vec()
    };
    Ok(PyDecimal { digits, exponent })
}
