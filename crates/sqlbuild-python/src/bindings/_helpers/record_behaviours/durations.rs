//! Calendar arithmetic and validation of the natively hosted cursor algebra records.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple, PyTupleMethods};

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::main::durations::parse_cursor_duration::parse_cursor_duration;
use sqlbuild_python_front::cursor::main::durations::shift_by_cursor_duration::shift_by_cursor_duration;
use sqlbuild_python_front::cursor::main::intervals::interval_validation::interval_validation;

/// `(field, unit suffix)` of each duration part, in declaration order.
const DURATION_UNITS: [(&str, &str); 6] = [
    ("years", "y"),
    ("months", "mo"),
    ("days", "d"),
    ("hours", "h"),
    ("minutes", "m"),
    ("seconds", "s"),
];
const MONTHS_PER_YEAR: i64 = 12;
const SECONDS_PER_DAY: i64 = 86_400;
const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_MINUTE: i64 = 60;

/// The one positional or keyword argument `name` of a native method call.
fn argument<'py>(
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(kwargs) = kwargs
        && let Some(value) = kwargs.get_item(name)?
    {
        return Ok(value);
    }
    args.get_item(0)
}

fn part(duration: &Bound<'_, PyAny>, name: &str) -> PyResult<i64> {
    native_field(duration, name)?.extract()
}

fn total_months(duration: &Bound<'_, PyAny>) -> PyResult<i64> {
    Ok(part(duration, "years")? * MONTHS_PER_YEAR + part(duration, "months")?)
}

fn fixed_seconds(duration: &Bound<'_, PyAny>) -> PyResult<i64> {
    Ok(part(duration, "days")? * SECONDS_PER_DAY
        + part(duration, "hours")? * SECONDS_PER_HOUR
        + part(duration, "minutes")? * SECONDS_PER_MINUTE
        + part(duration, "seconds")?)
}

fn units<'py>(duration: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyFrozenSet>> {
    let mut used: Vec<&str> = Vec::new();
    for (field, unit) in DURATION_UNITS {
        if part(duration, field)? != 0 {
            used.push(unit);
        }
    }
    PyFrozenSet::new(duration.py(), used)
}

/// `Duration.parse(value)`: the duration a text like `1y6mo` or `30d` spells, or `None`.
pub(crate) fn duration_parse(
    class: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = class.py();
    let value: String = argument(args, kwargs, "value")?.extract()?;
    let Some(parts) = parse_cursor_duration(py, &value)? else {
        return Ok(py.None());
    };
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    for ((field, _), amount) in DURATION_UNITS.iter().zip(parts.iter()) {
        keywords.set_item(*field, amount)?;
    }
    Ok(class.call((), Some(&keywords))?.unbind())
}

/// `Duration.is_zero`.
pub(crate) fn duration_is_zero(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    (total_months(instance)? == 0 && fixed_seconds(instance)? == 0).into_py_any(instance.py())
}

/// `Duration.units`: the unit suffixes with non-zero values.
pub(crate) fn duration_units(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(units(instance)?.into_any().unbind())
}

/// `Duration.has_calendar_component`: whether years or months are used.
pub(crate) fn duration_has_calendar_component(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    (total_months(instance)? != 0).into_py_any(instance.py())
}

/// `Duration.fixed_seconds`: days and below as whole seconds.
pub(crate) fn duration_fixed_seconds(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    fixed_seconds(instance)?.into_py_any(instance.py())
}

/// `Duration.total_months`: years folded into months.
pub(crate) fn duration_total_months(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    total_months(instance)?.into_py_any(instance.py())
}

/// `Duration.is_single_unit_in(allowed_units)`.
pub(crate) fn duration_is_single_unit_in(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let allowed: Bound<'_, PyAny> = argument(args, kwargs, "allowed_units")?;
    let used: Bound<'_, PyFrozenSet> = units(instance)?;
    let single: bool = used.len()? == 1 && used.as_any().le(&allowed)?;
    single.into_py_any(instance.py())
}

/// `Duration.add_to(moment)`.
pub(crate) fn duration_add_to(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let moment: Bound<'_, PyAny> = argument(args, kwargs, "moment")?;
    Ok(shift_by_cursor_duration(&moment, instance, 1)?.unbind())
}

/// `Duration.subtract_from(moment)`.
pub(crate) fn duration_subtract_from(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let moment: Bound<'_, PyAny> = argument(args, kwargs, "moment")?;
    Ok(shift_by_cursor_duration(&moment, instance, -1)?.unbind())
}

/// `AlignedInterval`: its start, end and grain must form a valid aligned interval.
pub(crate) fn finish_aligned_interval(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    interval_validation(values[0].bind(py), values[1].bind(py), values[2].bind(py))?;
    Ok(values)
}
