//! Microbatch counts, capped ranges and batch-limit configuration.

use std::marker::PhantomData;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyInt, PyString, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_analysis::planner_strategy::main::cap_integer_bounds::cap_integer_bounds;
use sqlbuild_analysis::planner_strategy::main::cap_timeline_bounds::cap_timeline_bounds;
use sqlbuild_analysis::planner_strategy::main::count_integer_batches::count_integer_batches;
use sqlbuild_analysis::planner_strategy::main::count_timeline_batches::count_timeline_batches;
use sqlbuild_analysis::planner_strategy::main::microbatch_limit_warning::microbatch_limit_warning as limit_warning;
use sqlbuild_analysis::planner_strategy::models::{CapDirection, CappedBound};
use sqlbuild_analysis::planner_strategy::types::Timeline;
use sqlbuild_cursor::algebra::main::parse_duration::parse_duration;
use sqlbuild_cursor::algebra::models::{DurationDigits, DurationSpan};

use crate::bindings::_helpers::cursor::durations::shifted_moment;
use crate::bindings::_helpers::plan_output::display::aligned_timestamp_bounds;
use crate::bindings::_helpers::planner_strategy::values::text;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

const INTEGER_CURSOR: &str = "integer";
const TIMESTAMP_CURSOR: &str = "timestamp";
const MONTHS_PER_YEAR: i64 = 12;
const CAP_FROM_START: &str = "cap_from_start";
const CAP_FROM_END: &str = "cap_from_end";
/// The limit actions that cap the planned range.
pub(crate) const CAP_ACTIONS: [&str; 2] = [CAP_FROM_END, CAP_FROM_START];

/// A start and end bound.
type BoundPair<'py> = (Bound<'py, PyAny>, Bound<'py, PyAny>);
/// An optional configured value.
type MaybeValue<'py> = Option<Bound<'py, PyAny>>;

/// Python `datetime` values one parsed duration apart.
struct DurationTimeline<'py> {
    span: DurationSpan,
    marker: PhantomData<Python<'py>>,
}

impl<'py> Timeline for DurationTimeline<'py> {
    type Moment = Bound<'py, PyAny>;
    type Error = PyErr;

    fn advance(&self, moment: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        shifted_moment(moment, &self.span, 1)
    }

    fn precedes(&self, left: &Bound<'py, PyAny>, right: &Bound<'py, PyAny>) -> PyResult<bool> {
        left.lt(right)
    }
}

/// `Duration.parse(batch_size)` as a span, or `None` when it is not a duration.
fn duration_span(batch_size: &str) -> Option<DurationSpan> {
    let digits: DurationDigits = parse_duration(batch_size)?;
    let number = |text: &str| -> Option<i64> {
        text.bytes().try_fold(0_i64, |value, digit| {
            value
                .checked_mul(10)?
                .checked_add(i64::from(digit.checked_sub(b'0')?))
        })
    };
    Some(DurationSpan {
        months: number(&digits.years)? * MONTHS_PER_YEAR + number(&digits.months)?,
        days: number(&digits.days)?,
        hours: number(&digits.hours)?,
        minutes: number(&digits.minutes)?,
        seconds: number(&digits.seconds)?,
    })
}

/// The bounds of a `CursorBounds` when each is an instance of one of the cursor value `classes`.
fn typed_bounds<'py>(
    bounds: &Bound<'py, PyAny>,
    classes: &[RecordId],
) -> PyResult<Option<BoundPair<'py>>> {
    let py: Python<'py> = bounds.py();
    let (start, end) = (native_field(bounds, "start")?, native_field(bounds, "end")?);
    for value in [&start, &end] {
        let mut typed: bool = false;
        for class in classes {
            typed = typed || value.is_instance(&record_class(py, *class)?)?;
        }
        if !typed {
            return Ok(None);
        }
    }
    Ok(Some((start, end)))
}

/// The integer values of integer bounds.
fn integer_bounds(bounds: &Bound<'_, PyAny>) -> PyResult<Option<(i128, i128)>> {
    let Some((start, end)) = typed_bounds(bounds, &[RecordId::IntegerValue])? else {
        return Ok(None);
    };
    Ok(Some((
        native_field(&start, "value")?.extract()?,
        native_field(&end, "value")?.extract()?,
    )))
}

/// The aligned `datetime` bounds of date or timestamp bounds and the batch timeline.
fn timeline_bounds<'py>(
    bounds: &Bound<'py, PyAny>,
    batch_size: &str,
) -> PyResult<Option<(DurationTimeline<'py>, BoundPair<'py>)>> {
    let Some(span) = duration_span(batch_size) else {
        return Ok(None);
    };
    let Some((start, end)) =
        typed_bounds(bounds, &[RecordId::DateValue, RecordId::TimestampValue])?
    else {
        return Ok(None);
    };
    let aligned: Bound<'py, PyTuple> = aligned_timestamp_bounds(&start, &end)?;
    Ok(Some((
        DurationTimeline {
            span,
            marker: PhantomData,
        },
        (aligned.get_item(0)?, aligned.get_item(1)?),
    )))
}

/// Batches in one concrete cursor range.
pub(crate) fn count_microbatches(
    bounds: &Bound<'_, PyAny>,
    batch_size: &str,
    cursor_type: &str,
    equal_bounds_are_batch: bool,
) -> PyResult<i128> {
    if cursor_type == INTEGER_CURSOR {
        return Ok(integer_bounds(bounds)?.map_or(0, |range| {
            count_integer_batches(range, batch_size, equal_bounds_are_batch)
        }));
    }
    if cursor_type != TIMESTAMP_CURSOR {
        return Ok(0);
    }
    let Some((timeline, range)) = timeline_bounds(bounds, batch_size)? else {
        return Ok(0);
    };
    count_timeline_batches(&timeline, range, equal_bounds_are_batch)
}

/// `CursorBounds(start=..., end=...)`.
fn cursor_bounds<'py>(
    start: Bound<'py, PyAny>,
    end: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = start.py();
    new_record_with(
        py,
        RecordId::CursorBounds,
        &[("start", start), ("end", end)],
    )
}

/// `<class>(value=value)` of the cursor values module.
fn cursor_value<'py>(
    py: Python<'py>,
    class: RecordId,
    value: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, class, &[("value", value)])
}

/// One concrete range capped to `max_batches` batch windows from the end the action keeps.
pub(crate) fn capped_microbatch_bounds<'py>(
    bounds: Bound<'py, PyAny>,
    (batch_size, cursor_type): (&str, &str),
    max_batches: i128,
    action: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = bounds.py();
    let direction: CapDirection = match action {
        CAP_FROM_START => CapDirection::FromStart,
        CAP_FROM_END => CapDirection::FromEnd,
        _ => return Ok(bounds),
    };
    let (start, end) = (
        native_field(&bounds, "start")?,
        native_field(&bounds, "end")?,
    );
    let capped: CappedBound<Bound<'py, PyAny>> = if cursor_type == INTEGER_CURSOR {
        let Some(range) = integer_bounds(&bounds)? else {
            return Ok(bounds);
        };
        let Some(capped) = cap_integer_bounds(range, batch_size, max_batches, direction) else {
            return Ok(bounds);
        };
        match capped {
            CappedBound::End(value) => CappedBound::End(cursor_value(
                py,
                RecordId::IntegerValue,
                value.into_pyobject(py)?.into_any(),
            )?),
            CappedBound::Start(value) => CappedBound::Start(cursor_value(
                py,
                RecordId::IntegerValue,
                value.into_pyobject(py)?.into_any(),
            )?),
        }
    } else {
        if cursor_type != TIMESTAMP_CURSOR {
            return Ok(bounds);
        }
        let Some((timeline, range)) = timeline_bounds(&bounds, batch_size)? else {
            return Ok(bounds);
        };
        match cap_timeline_bounds(&timeline, range, max_batches, direction)? {
            CappedBound::End(value) => {
                CappedBound::End(cursor_value(py, RecordId::TimestampValue, value)?)
            }
            CappedBound::Start(value) => {
                CappedBound::Start(cursor_value(py, RecordId::TimestampValue, value)?)
            }
        }
    };
    match capped {
        CappedBound::End(capped_end) => cursor_bounds(start, capped_end),
        CappedBound::Start(capped_start) => cursor_bounds(capped_start, end),
    }
}

/// The prominent warning or error message when one model exceeds its batch limit.
#[pyfunction]
#[pyo3(signature = (*, model_name, max_batches, batch_count, action))]
fn microbatch_limit_warning(
    model_name: &str,
    max_batches: Option<i128>,
    batch_count: i128,
    action: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let action: String = text(action)?;
    let cap: bool = CAP_ACTIONS.contains(&action.as_str());
    Ok(limit_warning(
        model_name,
        (max_batches, batch_count),
        (&action, cap),
    ))
}

/// A positive non-boolean integer, or `None`.
fn positive_integer<'py>(value: Option<Bound<'py, PyAny>>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_instance_of::<PyInt>() && !value.is_instance_of::<PyBool>() && value.gt(0)? {
        return Ok(Some(value));
    }
    Ok(None)
}

/// The configured batch limit and explicit action of validated model config values.
pub(crate) fn microbatch_limit_config<'py>(
    values: &Bound<'py, PyAny>,
) -> PyResult<(MaybeValue<'py>, MaybeValue<'py>)> {
    let py: Python<'py> = values.py();
    let raw: Bound<'py, PyAny> = mapping_get(values, "microbatch_limit")?;
    if let Ok(raw) = raw.downcast::<PyDict>() {
        let max_batches: Option<Bound<'py, PyAny>> =
            positive_integer(raw.get_item("max_batches")?)?;
        let action: Option<Bound<'py, PyAny>> = raw
            .get_item("action")?
            .filter(|action| action.is_instance_of::<PyString>());
        if let (Some(max_batches), Some(action)) = (max_batches, action) {
            let action: Bound<'py, PyAny> =
                enum_from_value(py, RecordId::MicrobatchLimitAction, action)?;
            return Ok((Some(max_batches), Some(action)));
        }
    }
    let legacy: Bound<'py, PyAny> = mapping_get(values, "max_microbatches")?;
    Ok((positive_integer(Some(legacy))?, None))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(microbatch_limit_warning, module)?)?;
    Ok(())
}
