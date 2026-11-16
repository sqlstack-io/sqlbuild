//! `--since` and `--until` as UTC datetimes, parsed with Python's ISO rules.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyInt};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cost_command::constants::ISO_DATE_LENGTH;
use sqlbuild_core::cost_command::errors::CostCommandError;
use sqlbuild_core::cost_command::main::is_zero_cost_duration::is_zero_cost_duration;

use crate::bindings::_helpers::cost_command::refusal::cost_error;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const DATETIME_MODULE: &str = "datetime";
const MINUTE_TO_DAY_DURATION_UNITS: [&str; 3] = ["m", "h", "d"];

fn utc(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "UTC")
}

fn timedelta<'py>(
    py: Python<'py>,
    unit: &str,
    amount: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item(unit, amount)?;
    imported(py, DATETIME_MODULE, "timedelta")?.call((), Some(&options))
}

fn now(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "datetime")?.call_method1("now", (utc(py)?,))
}

fn invalid(py: Python<'_>, label: &str, error: PyErr) -> PyErr {
    let wrapped: PyErr = cost_error(
        py,
        &CostCommandError::new(
            "C707",
            format!("cost {label} must be an ISO date or datetime"),
        ),
    );
    wrapped.set_cause(py, Some(error));
    wrapped
}

/// An ISO date (its UTC start, or the next day's for an end bound) or a zoned datetime.
pub(crate) fn parse_bound<'py>(
    py: Python<'py>,
    value: &str,
    label: &str,
    end_of_day: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let parsed: PyResult<Bound<'py, PyAny>> = (|| {
        if value.chars().count() == ISO_DATE_LENGTH {
            let day: Bound<'py, PyAny> =
                imported(py, DATETIME_MODULE, "date")?.call_method1("fromisoformat", (value,))?;
            let options: Bound<'py, PyDict> = PyDict::new(py);
            options.set_item("tzinfo", utc(py)?)?;
            let start: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?.call_method(
                "combine",
                (day, imported(py, DATETIME_MODULE, "time")?.getattr("min")?),
                Some(&options),
            )?;
            if end_of_day {
                return start.add(timedelta(py, "days", PyInt::new(py, 1).into_any())?);
            }
            return Ok(start);
        }
        imported(py, DATETIME_MODULE, "datetime")?
            .call_method1("fromisoformat", (value.replace('Z', "+00:00"),))
    })();
    let parsed: Bound<'py, PyAny> = match parsed {
        Ok(parsed) => parsed,
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            return Err(invalid(py, label, error));
        }
        Err(error) => return Err(error),
    };
    if value.chars().count() == ISO_DATE_LENGTH {
        return Ok(parsed);
    }
    if native_field(&parsed, "tzinfo")?.is_none() {
        return Err(cost_error(
            py,
            &CostCommandError::new(
                "C708",
                format!("cost {label} datetime must include a timezone"),
            ),
        ));
    }
    parsed.call_method1("astimezone", (utc(py)?,))
}

/// `--since`: now for a zero window, a relative day/hour/minute window, or an ISO bound.
pub(crate) fn parse_since<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    if is_zero_cost_duration(value) {
        return now(py);
    }
    let duration: Bound<'py, PyAny> =
        record_class(py, RecordId::Duration)?.call_method1("parse", (value,))?;
    if duration.is_none() {
        return parse_bound(py, value, "--since", false);
    }
    let units: Bound<'py, PyAny> = PyFrozenSet::new(py, MINUTE_TO_DAY_DURATION_UNITS)?.into_any();
    if !duration
        .call_method1("is_single_unit_in", (units,))?
        .is_truthy()?
    {
        return Err(cost_error(
            py,
            &CostCommandError::new(
                "C707",
                "cost --since relative duration must use one of: d, h, m",
            ),
        ));
    }
    now(py)?.sub(timedelta(
        py,
        "seconds",
        native_field(&duration, "fixed_seconds")?,
    )?)
}

/// Parse a `cost --since` value into a UTC datetime.
#[pyfunction]
#[pyo3(signature = (*, value))]
fn parse_cost_since_bound<'py>(
    py: Python<'py>,
    value: Option<&str>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    value.map(|value| parse_since(py, value)).transpose()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_cost_since_bound, module)?)?;
    Ok(())
}
