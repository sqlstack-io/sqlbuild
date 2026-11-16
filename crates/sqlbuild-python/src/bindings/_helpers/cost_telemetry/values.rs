//! Python `Decimal` and `datetime` values as exact native decimals and microsecond instants.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_core::cost_allocation::models::PyDecimal;

use crate::bindings::_helpers::cursor::values::DATETIME_MODULE;
use crate::bindings::_helpers::state_rows::timestamps::offset_aware;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DECIMAL_MODULE: &str = "decimal";
/// The methods an adapter with warehouse cost telemetry implements.
const COST_AWARE_METHODS: [&str; 2] = ["cost_capability", "collect_run_cost"];

/// Whether `adapter` implements the cost telemetry methods.
pub(crate) fn cost_aware(adapter: &Bound<'_, PyAny>) -> PyResult<bool> {
    for name in COST_AWARE_METHODS {
        if !adapter.hasattr(name)? || native_field(adapter, name)?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A finite non-negative `Decimal` as its exact digits and exponent.
pub(crate) fn native_decimal(value: &Bound<'_, PyAny>) -> PyResult<PyDecimal> {
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

/// `Decimal((0, digits, exponent))`.
pub(crate) fn python_decimal<'py>(
    py: Python<'py>,
    value: &PyDecimal,
) -> PyResult<Bound<'py, PyAny>> {
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

/// Microseconds since the epoch: UTC for aware values, the wall clock for naive ones.
pub(crate) fn instant_micros(value: &Bound<'_, PyAny>) -> PyResult<i128> {
    let py: Python<'_> = value.py();
    let datetime: Bound<'_, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let epoch: Bound<'_, PyAny> = if offset_aware(value)? {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("tzinfo", imported(py, DATETIME_MODULE, "UTC")?)?;
        datetime.call((1970, 1, 1), Some(&options))?
    } else {
        datetime.call1((1970, 1, 1))?
    };
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("microseconds", 1)?;
    let micro: Bound<'_, PyAny> =
        imported(py, DATETIME_MODULE, "timedelta")?.call((), Some(&options))?;
    value.sub(epoch)?.floor_div(micro)?.extract()
}

/// The process-wide Python object in `cell`, built by `build` on first use.
pub(crate) fn process_object<'py>(
    cell: &'static PyOnceLock<Py<PyAny>>,
    py: Python<'py>,
    build: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<&'py Bound<'py, PyAny>> {
    cell.get_or_try_init(py, || Ok::<Py<PyAny>, PyErr>(build()?.unbind()))
        .map(|object| object.bind(py))
}
