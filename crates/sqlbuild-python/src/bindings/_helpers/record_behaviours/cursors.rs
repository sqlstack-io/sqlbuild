//! Cursor value normalization of the natively hosted planner cursor records.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_host::entries::models::ExecutorHost;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::scalars::parse_value::parse_value;

/// One cursor value conversion.
type Conversion = for<'py> fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>;

const SCALAR_CLASSES: [RecordId; 3] = [
    RecordId::TimestampValue,
    RecordId::DateValue,
    RecordId::IntegerValue,
];
const CURSOR_ERRORS_MODULE: &str = "sqlbuild.cursor_algebra.exceptions";
const CURSOR_ALGEBRA_ERROR: &str = "CursorAlgebraError";

/// A cursor scalar from `raw`: typed values unchanged, text read as an integer, else a timestamp.
pub(crate) fn inferred<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw.py();
    for class in SCALAR_CLASSES {
        if raw.is_instance(&record_class(py, class)?)? {
            return Ok(raw.clone());
        }
    }
    let text: Bound<'py, PyString> = raw.str()?;
    match parse_value(text.as_any(), &PyString::new(py, "integer").into_any()) {
        Ok(value) => Ok(value),
        Err(error)
            if error.is_instance(
                py,
                &imported(py, CURSOR_ERRORS_MODULE, CURSOR_ALGEBRA_ERROR)?,
            ) =>
        {
            parse_value(text.as_any(), &PyString::new(py, "timestamp").into_any())
        }
        Err(error) => Err(error),
    }
}

/// `inferred(raw)`, keeping `None`.
pub(crate) fn optional_inferred<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if raw.is_none() {
        return Ok(raw.clone());
    }
    inferred(raw)
}

/// A cursor bound: a sentinel or its token text as the sentinel, else an inferred scalar.
fn bound<'py>(raw: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let sentinel: Bound<'py, PyAny> = host_entry(raw.py(), ExecutorHost::BoundSentinel)?;
    if raw.is_instance(&sentinel)? {
        return Ok(raw.clone());
    }
    for member in sentinel.try_iter()? {
        let member: Bound<'py, PyAny> = member?;
        if raw.eq(native_field(&member, "value")?)? {
            return Ok(member);
        }
    }
    inferred(raw)
}

/// `values` with `convert` applied at each of `indexes`.
fn converted(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
    (indexes, convert): (&[usize], Conversion),
) -> PyResult<Vec<Py<PyAny>>> {
    for index in indexes {
        let value: Bound<'_, PyAny> = convert(values[*index].bind(py))?;
        values[*index] = value.unbind();
    }
    Ok(values)
}

/// `values` with the items at each of `indexes` read as a tuple of cursor scalars.
fn converted_tuples(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
    indexes: &[usize],
) -> PyResult<Vec<Py<PyAny>>> {
    for index in indexes {
        let items: Vec<Bound<'_, PyAny>> = values[*index]
            .bind(py)
            .try_iter()?
            .map(|item| inferred(&item?))
            .collect::<PyResult<_>>()?;
        values[*index] = PyTuple::new(py, items)?.into_any().unbind();
    }
    Ok(values)
}

/// `ModelCursorSnapshot`: watermark values parsed as cursor scalars.
pub(crate) fn finish_cursor_snapshot(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let mut values: Vec<Py<PyAny>> = converted(py, values, (&[0, 3, 4], optional_inferred))?;
    values = converted_tuples(py, values, &[1, 2, 11, 12, 14])?;
    let pairs: Vec<Bound<'_, PyTuple>> = values[13]
        .bind(py)
        .try_iter()?
        .map(|pair| {
            let pair: Bound<'_, PyAny> = pair?;
            PyTuple::new(
                py,
                [
                    optional_inferred(&pair.get_item(0)?)?,
                    optional_inferred(&pair.get_item(1)?)?,
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    values[13] = PyTuple::new(py, pairs)?.into_any().unbind();
    let mode: Bound<'_, PyAny> =
        enum_from_value(py, RecordId::CursorWatermarkMode, values[10].bind(py))?;
    values[10] = mode.unbind();
    Ok(values)
}

/// `ModelCursorSnapshot.watermarks_available`: whether every physical watermark was read.
pub(crate) fn snapshot_watermarks_available(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let available: bool = !native_field(instance, "unavailable_watermark_tags")?.is_truthy()?;
    Ok(pyo3::types::PyBool::new(instance.py(), available)
        .to_owned()
        .into_any()
        .unbind())
}

/// `CursorBounds`: start and end read as bounds.
pub(crate) fn finish_cursor_bounds(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    converted(py, values, (&[0, 1], bound))
}

/// `CursorInputEvidence`: an optional minimum and a maximum scalar.
pub(crate) fn finish_input_evidence(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let values: Vec<Py<PyAny>> = converted(py, values, (&[2], optional_inferred))?;
    converted(py, values, (&[3], inferred))
}

/// `FutureCursorSafetyEvidence`: its seven cursor values read as scalars.
pub(crate) fn finish_future_evidence(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    converted(py, values, (&[2, 3, 4, 5, 6, 7, 8], inferred))
}

/// `MaximumStartSafetyEvidence`: its cursor values read as scalars.
pub(crate) fn finish_maximum_evidence(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let values: Vec<Py<PyAny>> = converted(py, values, (&[2, 3, 5, 6], inferred))?;
    converted(py, values, (&[4], optional_inferred))
}

/// `CursorInputRelation`: its grain as a `CursorGrain` and its terminal bounds as scalars.
pub(crate) fn finish_input_relation(
    py: Python<'_>,
    mut values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    if !values[2].bind(py).is_none() {
        let grain: Bound<'_, PyAny> =
            enum_from_value(py, RecordId::CursorGrain, values[2].bind(py))?;
        values[2] = grain.unbind();
    }
    converted(py, values, (&[5, 6], optional_inferred))
}

/// `CursorInputRelation.is_runtime_owned`: whether bounds are read after upstream execution.
pub(crate) fn input_relation_runtime_owned(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(instance, "is_runtime_produced")?.unbind())
}
