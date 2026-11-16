//! Parsing, rendering, comparing and stepping typed cursor values.

use crate::cursor::main::conversion::algebra_error::algebra_error;
use crate::cursor::main::scalars::as_scalar::as_scalar;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_cursor::algebra::errors::CursorError;
use sqlbuild_cursor::algebra::main::cursor_sort_key::cursor_sort_key as sort_key;
use sqlbuild_cursor::algebra::main::exclusive_end_from_observed_max::exclusive_end_from_observed_max;
use sqlbuild_cursor::algebra::main::extreme_position::extreme_position;
use sqlbuild_cursor::algebra::main::floor_to_grain::floor_to_grain;
use sqlbuild_cursor::algebra::main::next_boundary::next_boundary;
use sqlbuild_cursor::algebra::main::normalize_timestamp_text::normalize_timestamp_text;
use sqlbuild_cursor::algebra::main::parse_integer_text::parse_integer_text;
use sqlbuild_cursor::algebra::models::{Extreme, Grain, Scalar, SortKey};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::cursor::constants::DATETIME_MODULE;
use crate::cursor::models::CursorSession;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const DECIMAL_MODULE: &str = "decimal";
pub(crate) const INTEGER_CURSOR: &str = "integer";
pub(crate) const TIMESTAMP_CURSOR: &str = "timestamp";

/// `CursorAlgebraError(message)` raised from `cause`.
fn chained(py: Python<'_>, message: String, cause: PyErr) -> PyErr {
    let error: PyErr = algebra_error(py, &message);
    error.set_cause(py, Some(cause));
    error
}

/// `IntegerValue` parsed from `str(raw)` the way `int(Decimal(str(raw)))` reads it.
pub(crate) fn parse_integer<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let text: String = raw.str()?.to_str()?.to_owned();
    if let Some(value) = parse_integer_text(&text) {
        return new_record_with(
            py,
            RecordId::IntegerValue,
            &[("value", value.into_pyobject(py)?.into_any())],
        );
    }
    let parsed: PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> =
        imported(py, DECIMAL_MODULE, "Decimal")?
            .call1((text.as_str(),))
            .and_then(|decimal| {
                let integer: Bound<'py, PyAny> =
                    py.get_type::<pyo3::types::PyInt>().call1((&decimal,))?;
                Ok((decimal, integer))
            });
    let (decimal, integer) = match parsed {
        Ok(values) => values,
        Err(error) if integer_failure(py, &error)? => {
            return Err(chained(
                py,
                format!("invalid integer cursor value: {text}"),
                error,
            ));
        }
        Err(error) => return Err(error),
    };
    if decimal.ne(&integer)? {
        return Err(algebra_error(
            py,
            &format!("non-integral integer cursor value: {text}"),
        ));
    }
    new_record_with(py, RecordId::IntegerValue, &[("value", integer)])
}

/// Whether `int(Decimal(...))` failed with `InvalidOperation`, `ValueError` or `OverflowError`.
fn integer_failure(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    let invalid: Bound<'_, PyAny> = imported(py, DECIMAL_MODULE, "InvalidOperation")?;
    Ok(error.is_instance(py, &invalid)
        || error.is_instance_of::<PyValueError>(py)
        || error.is_instance_of::<pyo3::exceptions::PyOverflowError>(py))
}

/// `value.astimezone(UTC)` for an aware `datetime`, else `value`.
pub(crate) fn utc_normalized<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if native_field(value, "tzinfo")?.is_none() {
        return Ok(value.clone());
    }
    value.call_method1(
        "astimezone",
        (imported(value.py(), DATETIME_MODULE, "UTC")?,),
    )
}

/// A `DateValue` or `TimestampValue` parsed from warehouse timestamp text.
pub(crate) fn parse_timestamp_text<'py>(
    raw: &Bound<'py, PyAny>,
    text: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    let normalized: String = normalize_timestamp_text(text);
    let as_date = imported(py, DATETIME_MODULE, "date")?
        .call_method1("fromisoformat", (normalized.as_str(),));
    match as_date {
        Ok(date) => {
            return new_record_with(py, RecordId::DateValue, &[("value", date)]);
        }
        Err(error) if error.is_instance_of::<PyValueError>(py) => {}
        Err(error) => return Err(error),
    }
    let parsed = imported(py, DATETIME_MODULE, "datetime")?
        .call_method1("fromisoformat", (normalized.as_str(),))
        .and_then(|value| utc_normalized(&value));
    match parsed {
        Ok(value) => new_record_with(
            py,
            RecordId::TimestampValue,
            &[
                ("value", value),
                ("source_text", PyString::new(py, &normalized).into_any()),
            ],
        ),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Err(chained(
            py,
            format!("invalid timestamp cursor value: {}", raw.str()?),
            error,
        )),
        Err(error) => Err(error),
    }
}

/// The sentinel a SQL token names, if it names one.
#[pyfunction(name = "cursor_sentinel_from_token")]
#[pyo3(signature = (*, token))]
pub(crate) fn sentinel_from_token<'py>(
    token: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = token.py();
    match host_entry(py, ExecutorHost::BoundSentinel)?.call1((token,)) {
        Ok(sentinel) => Ok(Some(sentinel)),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

fn extreme_bound<'py>(
    values: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    extreme: Extreme,
) -> PyResult<Bound<'py, PyAny>> {
    let session: CursorSession<'py> = CursorSession::new(values.py())?;
    let raws: Vec<Bound<'py, PyAny>> = values.try_iter()?.collect::<PyResult<_>>()?;
    let mut keys: Vec<SortKey> = Vec::with_capacity(raws.len());
    for raw in &raws {
        let scalar: Scalar = as_scalar(&session, raw, cursor_type)?;
        let key: SortKey = session.checked(sort_key(&scalar, &session))?;
        let unorderable: bool = keys
            .first()
            .is_some_and(|first| std::mem::discriminant(first) != std::mem::discriminant(&key));
        keys.push(key);
        if unorderable {
            break;
        }
    }
    let position: usize = session.checked(extreme_position(&keys, extreme))?;
    Ok(raws[position].clone())
}

/// The numerically or temporally largest raw bound.
#[pyfunction(name = "cursor_max_bound")]
#[pyo3(signature = (*, values, cursor_type))]
pub(crate) fn max_bound<'py>(
    values: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    extreme_bound(values, cursor_type, Extreme::Max)
}

/// The numerically or temporally smallest raw bound.
#[pyfunction(name = "cursor_min_bound")]
#[pyo3(signature = (*, values, cursor_type))]
pub(crate) fn min_bound<'py>(
    values: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    extreme_bound(values, cursor_type, Extreme::Min)
}

/// A scalar stepped by one grain-dependent operation.
pub(crate) fn stepped<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
    step: impl FnOnce(&Scalar, Option<Grain>) -> Result<Scalar, CursorError>,
) -> PyResult<Bound<'py, PyAny>> {
    let session: CursorSession<'py> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    let result: Scalar = session.checked(step(&scalar, session.grain(grain)?))?;
    session.python_scalar(&result)
}

/// The aligned boundary at or before a cursor value.
#[pyfunction(name = "cursor_floor_to_grain")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn floor_value<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, |scalar, grain| {
        Ok(floor_to_grain(scalar, grain))
    })
}

/// The boundary one grain after an aligned cursor value.
#[pyfunction(name = "cursor_next_boundary")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn next_value<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, next_boundary)
}

/// The exclusive end of the partition holding an observed maximum.
#[pyfunction(name = "cursor_exclusive_end_from_observed_max")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn observed_end<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let session: CursorSession<'py> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    let end: Scalar = session.checked(exclusive_end_from_observed_max(
        &scalar,
        session.grain(grain)?,
        &session,
    ))?;
    session.python_scalar(&end)
}
