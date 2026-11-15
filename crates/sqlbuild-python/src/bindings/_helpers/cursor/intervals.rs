//! Building, intersecting, merging, splitting and capping half-open cursor intervals.

use std::cmp::Ordering;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PySlice, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cursor::algebra::main::cap_from_end::cap_from_end;
use sqlbuild_cursor::algebra::main::cap_from_start::cap_from_start;
use sqlbuild_cursor::algebra::main::clamp::clamp;
use sqlbuild_cursor::algebra::main::compare::compare;
use sqlbuild_cursor::algebra::main::merge::merge;
use sqlbuild_cursor::algebra::main::observed_partition::observed_partition;
use sqlbuild_cursor::algebra::main::split_into_batches::split_into_batches;
use sqlbuild_cursor::algebra::main::validate_interval::validate_interval;
use sqlbuild_cursor::algebra::models::{Interval, Scalar};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::cursor::conversion::CursorSession;
use crate::bindings::_helpers::cursor::scalars::as_scalar;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const RAW_INTERVAL_CURSOR_TYPES: [&str; 2] = ["timestamp", "integer"];

/// One stored interval with its parsed bounds.
struct RawInterval<'py> {
    start: Scalar,
    end: Scalar,
    interval: Bound<'py, PyAny>,
}

fn python_intervals<'py>(
    session: &CursorSession<'py>,
    intervals: &[Interval],
) -> PyResult<Bound<'py, PyTuple>> {
    let built: Vec<Bound<'py, PyAny>> = intervals
        .iter()
        .map(|interval| session.python_interval(interval))
        .collect::<PyResult<_>>()?;
    PyTuple::new(session.py(), built)
}

/// Raise `CursorAlgebraError` unless the ends form a valid aligned interval.
#[pyfunction(name = "validate_cursor_interval")]
#[pyo3(signature = (*, start, end, grain))]
pub(crate) fn interval_validation(
    start: &Bound<'_, PyAny>,
    end: &Bound<'_, PyAny>,
    grain: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let session: CursorSession<'_> = CursorSession::new(start.py())?;
    let interval = Interval {
        start: session.scalar(start)?,
        end: session.scalar(end)?,
        grain: session.grain(grain)?,
    };
    session.checked(validate_interval(&interval, &session))
}

/// The canonical partition containing an observed physical value.
#[pyfunction(name = "cursor_observed_partition")]
#[pyo3(signature = (*, value, grain))]
pub(crate) fn partition<'py>(
    value: &Bound<'py, PyAny>,
    grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let session: CursorSession<'py> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    let interval: Interval = session.checked(observed_partition(&scalar, session.grain(grain)?))?;
    session.python_interval(&interval)
}

/// The intersection of two compatible half-open intervals, or `None`.
#[pyfunction(name = "cursor_clamp")]
#[pyo3(signature = (*, interval, bounds))]
fn clamp_intervals<'py>(
    interval: &Bound<'py, PyAny>,
    bounds: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let session: CursorSession<'py> = CursorSession::new(interval.py())?;
    let (interval, bounds): (Interval, Interval) =
        (session.interval(interval)?, session.interval(bounds)?);
    session
        .checked(clamp(&interval, &bounds, &session))?
        .map(|clamped| session.python_interval(&clamped))
        .transpose()
}

/// Overlapping or adjacent canonical intervals merged.
#[pyfunction(name = "cursor_merge")]
#[pyo3(signature = (*, intervals))]
fn merge_intervals<'py>(intervals: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let session: CursorSession<'py> = CursorSession::new(intervals.py())?;
    let typed: Vec<Interval> = intervals
        .try_iter()?
        .map(|interval| session.interval(&interval?))
        .collect::<PyResult<_>>()?;
    let merged: Vec<Interval> = session.checked(merge(&typed, &session))?;
    python_intervals(&session, &merged)
}

/// Overlapping or adjacent raw microbatch intervals merged, keeping disjoint ones in order.
#[pyfunction(name = "merge_microbatch_intervals")]
#[pyo3(signature = (*, intervals, cursor_type))]
fn merge_raw_intervals<'py>(
    intervals: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = intervals.py();
    let kind: String = cursor_type.str()?.extract()?;
    if !RAW_INTERVAL_CURSOR_TYPES.contains(&kind.as_str()) {
        return Err(host_entry(py, ExecutorHost::MicrobatchStateError)
            .and_then(|class| class.call1((format!("unsupported cursor type: {kind}"),)))
            .map_or_else(|error| error, PyErr::from_value));
    }
    let session: CursorSession<'py> = CursorSession::new(py)?;
    let mut parsed: Vec<RawInterval<'py>> = Vec::new();
    for interval in intervals.try_iter()? {
        let interval: Bound<'py, PyAny> = interval?;
        parsed.push(RawInterval {
            start: as_scalar(&session, &native_field(&interval, "start")?, cursor_type)?,
            end: as_scalar(&session, &native_field(&interval, "end")?, cursor_type)?,
            interval,
        });
    }
    let mut failure: Option<PyErr> = None;
    parsed.sort_by(|left, right| {
        session
            .checked(compare(&left.start, &right.start, &session))
            .unwrap_or_else(|error| {
                failure.get_or_insert(error);
                Ordering::Equal
            })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    let class: Bound<'py, PyAny> = host_entry(py, ExecutorHost::MicrobatchInterval)?;
    let mut merged: Vec<RawInterval<'py>> = Vec::new();
    for next in parsed {
        let Some(last) = merged.last_mut() else {
            merged.push(next);
            continue;
        };
        if session.checked(compare(&last.end, &next.start, &session))? == Ordering::Less {
            merged.push(next);
            continue;
        }
        if session.checked(compare(&last.end, &next.end, &session))? == Ordering::Less {
            last.interval = call_with(
                &class,
                &[
                    ("start", native_field(&last.interval, "start")?),
                    ("end", native_field(&next.interval, "end")?),
                ],
            )?;
            last.end = next.end;
        }
    }
    PyTuple::new(py, merged.into_iter().map(|raw| raw.interval))
}

/// An interval split into batches of a positive number of grain units.
#[pyfunction(name = "cursor_split_into_batches")]
#[pyo3(signature = (*, interval, step))]
fn split<'py>(interval: &Bound<'py, PyAny>, step: i64) -> PyResult<Bound<'py, PyTuple>> {
    let session: CursorSession<'py> = CursorSession::new(interval.py())?;
    let typed: Interval = session.interval(interval)?;
    let batches: Vec<Interval> = session.checked(split_into_batches(&typed, step, &session))?;
    python_intervals(&session, &batches)
}

fn sliced<'py>(
    intervals: &Bound<'py, PyAny>,
    range: std::ops::Range<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let slice = PySlice::new(intervals.py(), range.start as isize, range.end as isize, 1);
    intervals.get_item(slice)
}

/// At most `count` intervals from the start.
#[pyfunction(name = "cursor_cap_from_start")]
#[pyo3(signature = (*, intervals, count))]
fn cap_start<'py>(intervals: &Bound<'py, PyAny>, count: i64) -> PyResult<Bound<'py, PyAny>> {
    sliced(intervals, cap_from_start(intervals.len()?, count))
}

/// At most `count` intervals from the end; an empty tuple when `count` is not positive.
#[pyfunction(name = "cursor_cap_from_end")]
#[pyo3(signature = (*, intervals, count))]
fn cap_end<'py>(intervals: &Bound<'py, PyAny>, count: i64) -> PyResult<Bound<'py, PyAny>> {
    if count <= 0 {
        return Ok(PyTuple::empty(intervals.py()).into_any());
    }
    sliced(intervals, cap_from_end(intervals.len()?, count))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(interval_validation, module)?)?;
    module.add_function(wrap_pyfunction!(partition, module)?)?;
    module.add_function(wrap_pyfunction!(clamp_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(merge_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(merge_raw_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(split, module)?)?;
    module.add_function(wrap_pyfunction!(cap_start, module)?)?;
    module.add_function(wrap_pyfunction!(cap_end, module)?)?;
    Ok(())
}
