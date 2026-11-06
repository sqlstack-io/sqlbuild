//! Parsing, rendering, comparing and stepping typed cursor values.

use std::cmp::Ordering;

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cursor::algebra::errors::CursorError;
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::algebra::main::cursor_sort_key::cursor_sort_key as sort_key;
use sqlbuild_cursor::algebra::main::exclusive_end_from_observed_max::exclusive_end_from_observed_max;
use sqlbuild_cursor::algebra::main::exclusive_to_inclusive::exclusive_to_inclusive;
use sqlbuild_cursor::algebra::main::extreme_position::extreme_position;
use sqlbuild_cursor::algebra::main::floor_to_grain::floor_to_grain;
use sqlbuild_cursor::algebra::main::inclusive_to_exclusive::inclusive_to_exclusive;
use sqlbuild_cursor::algebra::main::next_boundary::next_boundary;
use sqlbuild_cursor::algebra::main::normalize_timestamp_text::normalize_timestamp_text;
use sqlbuild_cursor::algebra::main::parse_integer_text::parse_integer_text;
use sqlbuild_cursor::algebra::main::render::render;
use sqlbuild_cursor::algebra::models::{Extreme, Grain, Scalar, SortKey};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::cursor::conversion::{CursorSession, algebra_error};
use crate::bindings::_helpers::cursor::values::DATETIME_MODULE;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const DECIMAL_MODULE: &str = "decimal";
const INTEGER_CURSOR: &str = "integer";
const TIMESTAMP_CURSOR: &str = "timestamp";

/// `CursorAlgebraError(message)` raised from `cause`.
fn chained(py: Python<'_>, message: String, cause: PyErr) -> PyErr {
    let error: PyErr = algebra_error(py, &message);
    error.set_cause(py, Some(cause));
    error
}

/// `IntegerValue` parsed from `str(raw)` the way `int(Decimal(str(raw)))` reads it.
fn parse_integer<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
fn utc_normalized<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if native_field(value, "tzinfo")?.is_none() {
        return Ok(value.clone());
    }
    value.call_method1(
        "astimezone",
        (imported(value.py(), DATETIME_MODULE, "UTC")?,),
    )
}

/// A `DateValue` or `TimestampValue` parsed from warehouse timestamp text.
fn parse_timestamp_text<'py>(raw: &Bound<'py, PyAny>, text: &str) -> PyResult<Bound<'py, PyAny>> {
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

/// One cursor value parsed from its boundary representation, keeping date-ness and offsets.
pub(crate) fn parse_value<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    if cursor_type.eq(INTEGER_CURSOR)? {
        return parse_integer(raw);
    }
    if !cursor_type.eq(TIMESTAMP_CURSOR)? {
        return Err(algebra_error(
            py,
            &format!("unsupported cursor type: {}", cursor_type.str()?),
        ));
    }
    if raw.is_instance(&imported(py, DATETIME_MODULE, "datetime")?)? {
        let value: Bound<'py, PyAny> = utc_normalized(raw)?;
        return new_record_with(py, RecordId::TimestampValue, &[("value", value)]);
    }
    if raw.is_instance(&imported(py, DATETIME_MODULE, "date")?)? {
        return new_record_with(py, RecordId::DateValue, &[("value", raw.clone())]);
    }
    let Ok(text) = raw.downcast::<PyString>() else {
        return Err(algebra_error(
            py,
            &format!("invalid timestamp cursor value: {}", raw.str()?),
        ));
    };
    parse_timestamp_text(raw, text.to_str()?)
}

/// `raw` unchanged when it is already a typed value, else parsed.
pub(crate) fn as_scalar<'py>(
    session: &CursorSession<'py>,
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Scalar> {
    if session.scalar_typed(raw)? {
        return session.scalar(raw);
    }
    session.scalar(&parse_value(raw, cursor_type)?)
}

/// One cursor scalar, preserving date-ness and timestamp offsets.
#[pyfunction(name = "cursor_parse")]
#[pyo3(signature = (*, raw, cursor_type))]
pub(crate) fn parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    parse_value(raw, cursor_type)
}

/// Discovery values that may be dates or timestamps, or `None` when they do not parse.
#[pyfunction(name = "cursor_try_parse")]
#[pyo3(signature = (*, raw, cursor_type))]
pub(crate) fn try_parse<'py>(
    raw: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = raw.py();
    match parse_value(raw, cursor_type) {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if error.is_instance_of::<PyTypeError>(py)
                || error.is_instance_of::<PyValueError>(py) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// A scalar in the existing cursor output format.
#[pyfunction(name = "cursor_render")]
#[pyo3(signature = (*, value))]
pub(crate) fn render_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let session: CursorSession<'_> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    session.checked(render(&scalar, &session))
}

/// A sentinel's token, or a scalar rendered.
#[pyfunction(name = "cursor_sentinel_to_token")]
#[pyo3(signature = (*, sentinel))]
pub(crate) fn sentinel_to_token(sentinel: &Bound<'_, PyAny>) -> PyResult<String> {
    if sentinel.is_instance(&host::entry(sentinel.py(), ExecutorHost::BoundSentinel)?)? {
        return native_field(sentinel, "value")?.extract();
    }
    render_value(sentinel)
}

/// The sentinel a SQL token names, if it names one.
#[pyfunction(name = "cursor_sentinel_from_token")]
#[pyo3(signature = (*, token))]
fn sentinel_from_token<'py>(token: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = token.py();
    match host::entry(py, ExecutorHost::BoundSentinel)?.call1((token,)) {
        Ok(sentinel) => Ok(Some(sentinel)),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

/// `-1`, `0` or `1` as `left` orders against `right`.
#[pyfunction(name = "cursor_compare")]
#[pyo3(signature = (*, left, right))]
pub(crate) fn compare_values(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<i8> {
    let session: CursorSession<'_> = CursorSession::new(left.py())?;
    let (left, right): (Scalar, Scalar) = (session.scalar(left)?, session.scalar(right)?);
    Ok(match session.checked(compare(&left, &right, &session))? {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
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
fn max_bound<'py>(
    values: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    extreme_bound(values, cursor_type, Extreme::Max)
}

/// The numerically or temporally smallest raw bound.
#[pyfunction(name = "cursor_min_bound")]
#[pyo3(signature = (*, values, cursor_type))]
fn min_bound<'py>(
    values: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    extreme_bound(values, cursor_type, Extreme::Min)
}

/// A scalar stepped by one grain-dependent operation.
fn stepped<'py>(
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
fn floor_value<'py>(
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
fn next_value<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, next_boundary)
}

/// An operator-declared inclusive bound advanced by one cursor unit.
#[pyfunction(name = "cursor_inclusive_to_exclusive")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn to_exclusive<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, inclusive_to_exclusive)
}

/// An exclusive bound moved back by one cursor unit.
#[pyfunction(name = "cursor_exclusive_to_inclusive")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn to_inclusive<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    stepped(value, grain, exclusive_to_inclusive)
}

/// The exclusive end of the partition holding an observed maximum.
#[pyfunction(name = "cursor_exclusive_end_from_observed_max")]
#[pyo3(signature = (*, value, grain))]
fn observed_end<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse, module)?)?;
    module.add_function(wrap_pyfunction!(try_parse, module)?)?;
    module.add_function(wrap_pyfunction!(render_value, module)?)?;
    module.add_function(wrap_pyfunction!(sentinel_to_token, module)?)?;
    module.add_function(wrap_pyfunction!(sentinel_from_token, module)?)?;
    module.add_function(wrap_pyfunction!(compare_values, module)?)?;
    module.add_function(wrap_pyfunction!(max_bound, module)?)?;
    module.add_function(wrap_pyfunction!(min_bound, module)?)?;
    module.add_function(wrap_pyfunction!(floor_value, module)?)?;
    module.add_function(wrap_pyfunction!(next_value, module)?)?;
    module.add_function(wrap_pyfunction!(to_exclusive, module)?)?;
    module.add_function(wrap_pyfunction!(to_inclusive, module)?)?;
    module.add_function(wrap_pyfunction!(observed_end, module)?)?;
    Ok(())
}
