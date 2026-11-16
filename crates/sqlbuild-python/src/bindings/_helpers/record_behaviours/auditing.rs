//! Validation of the natively hosted measurement audit records.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyFloat};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_model_config::header_metadata::main::check_measurement_contract::check_measurement_contract;
use sqlbuild_model_config::header_metadata::main::check_threshold_bound::check_threshold_bound;
use sqlbuild_model_config::header_metadata::main::check_threshold_pair::check_threshold_pair;
use sqlbuild_model_config::header_metadata::models::{ThresholdBound, ThresholdInput};

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// The `MeasurementAuditError` carrying `message`.
fn measurement_error(py: Python<'_>, message: &str) -> PyErr {
    new_plain_error(py, ErrorId::MeasurementAudit, message)
}

/// How Python's threshold validation sees one limit value.
fn threshold_input(value: &Bound<'_, PyAny>) -> PyResult<ThresholdInput> {
    if value.is_none() {
        return Ok(ThresholdInput::Absent);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(ThresholdInput::Boolean);
    }
    Ok(ThresholdInput::Number(value.extract::<f64>()?))
}

/// `MeasurementThresholdBound`'s operator, limit, lower and upper, checked and made floats.
pub(crate) fn finish_threshold_bound(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let operator: String = values[0].bind(py).str()?.extract()?;
    let inputs = (
        threshold_input(values[1].bind(py))?,
        threshold_input(values[2].bind(py))?,
        threshold_input(values[3].bind(py))?,
    );
    let float = |value: f64| PyFloat::new(py, value).into_any().unbind();
    let none = || py.None();
    let (limit, lower, upper) = match check_threshold_bound(&operator, inputs) {
        Ok(ThresholdBound::Below(limit) | ThresholdBound::Above(limit)) => {
            (float(limit), none(), none())
        }
        Ok(ThresholdBound::Outside(lower, upper)) => (none(), float(lower), float(upper)),
        Err(message) => return Err(measurement_error(py, &message)),
    };
    let mut values: Vec<Py<PyAny>> = values;
    values[1] = limit;
    values[2] = lower;
    values[3] = upper;
    Ok(values)
}

/// The checked bound a `MeasurementThresholdBound` record holds, or `None`.
fn record_bound(bound: &Bound<'_, PyAny>) -> PyResult<Option<ThresholdBound>> {
    if bound.is_none() {
        return Ok(None);
    }
    let operator: String = native_field(bound, "operator")?.str()?.extract()?;
    let inputs = (
        threshold_input(&native_field(bound, "limit")?)?,
        threshold_input(&native_field(bound, "lower")?)?,
        threshold_input(&native_field(bound, "upper")?)?,
    );
    check_threshold_bound(&operator, inputs)
        .map(Some)
        .map_err(|message| measurement_error(bound.py(), &message))
}

/// `MeasurementThresholds`' warn and error bounds, checked as one policy.
pub(crate) fn finish_thresholds(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let warn: Option<ThresholdBound> = record_bound(values[0].bind(py))?;
    let error: Option<ThresholdBound> = record_bound(values[1].bind(py))?;
    check_threshold_pair(warn, error).map_err(|message| measurement_error(py, message))?;
    Ok(values)
}

/// `MeasurementContract`'s named columns and unit, checked non-blank.
pub(crate) fn finish_measurement_contract(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let value_column: String = values[0].bind(py).extract()?;
    let sample_count_column: Option<String> = values[1].bind(py).extract()?;
    let sample_unit: Option<String> = values[2].bind(py).extract()?;
    check_measurement_contract(
        &value_column,
        sample_count_column.as_deref(),
        sample_unit.as_deref(),
    )
    .map_err(|message| measurement_error(py, message))?;
    Ok(values)
}
