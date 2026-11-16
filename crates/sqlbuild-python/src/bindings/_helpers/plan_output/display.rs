//! Plan and progress display fragments over planner entries and cursor values.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::plan_display::constants::{
    DEFAULT_CURSOR_GRAIN, INTEGER_CURSOR, TIMESTAMP_CURSOR,
};
use sqlbuild_core::plan_display::main::cursor_bound_display::cursor_bound_display as bound_display;
use sqlbuild_core::plan_display::main::effective_microbatch_batch_size::effective_microbatch_batch_size;
use sqlbuild_core::plan_display::main::format_no_input_rows::format_no_input_rows as no_input_rows;
use sqlbuild_core::plan_display::main::format_waiting_on_empty_inputs::format_waiting_on_empty_inputs as waiting_on_empty_inputs;
use sqlbuild_core::plan_display::main::model_materialization_label::model_materialization_label as materialization_label;
use sqlbuild_core::plan_display::models::{ModelLabelFacts, ParsedBound};

use crate::bindings::_helpers::cursor::scalars::{
    render_value, to_exclusive, to_inclusive, try_parse,
};
use crate::bindings::_helpers::plan_output::values::{optional_text, text};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const DATETIME_MODULE: &str = "datetime";

fn label_facts(entry: &Bound<'_, PyAny>) -> PyResult<ModelLabelFacts> {
    let field =
        |name: &str| -> PyResult<Option<String>> { optional_text(&native_field(entry, name)?) };
    Ok(ModelLabelFacts {
        materialization_type: text(&native_field(entry, "materialization_type")?)?,
        action: text(&native_field(entry, "action")?)?,
        custom_materialization_name: field("custom_materialization_name")?,
        incremental_strategy: field("incremental_strategy")?,
        incremental_mode: field("incremental_mode")?,
        cursor_type: field("cursor_type")?,
        snapshot_strategy: field("snapshot_strategy")?,
        historical: !native_field(entry, "observed_at_column")?.is_none(),
        historical_input: field("historical_input")?,
    })
}

pub(crate) fn optional_facts(entry: &Bound<'_, PyAny>) -> PyResult<Option<ModelLabelFacts>> {
    if entry.is_none() {
        return Ok(None);
    }
    label_facts(entry).map(Some)
}

/// The cursor type, defaulting to timestamp, and its grain object (`None` for integers).
fn cursor_typing<'py>(
    cursor_type: &Bound<'py, PyAny>,
    cursor_grain: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = cursor_type.py();
    let effective: Bound<'py, PyAny> = if cursor_type.is_truthy()? {
        cursor_type.clone()
    } else {
        PyString::new(py, TIMESTAMP_CURSOR).into_any()
    };
    if effective.eq(INTEGER_CURSOR)? {
        return Ok((effective, py.None().into_bound(py)));
    }
    let grain: Bound<'py, PyAny> = if cursor_grain.is_truthy()? {
        cursor_grain.clone()
    } else {
        PyString::new(py, DEFAULT_CURSOR_GRAIN).into_any()
    };
    let grain: Bound<'py, PyAny> = enum_from_value(py, RecordId::CursorGrain, grain)?;
    Ok((effective, grain))
}

/// The last cursor value included by an exclusive end bound.
#[pyfunction]
#[pyo3(signature = (*, end, cursor_type, cursor_grain))]
pub(crate) fn inclusive_cursor_end(
    end: &Bound<'_, PyAny>,
    cursor_type: &Bound<'_, PyAny>,
    cursor_grain: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let (effective, grain) = cursor_typing(cursor_type, cursor_grain)?;
    let Some(parsed) = try_parse(end, &effective)? else {
        return end.extract();
    };
    render_value(&to_inclusive(&parsed, &grain)?)
}

/// A fully bounded inclusive operator override as an executable interval.
pub(crate) fn resolve_bounded_cursor_override<'py>(
    start_cursor_override: &Bound<'py, PyAny>,
    end_cursor_override: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    cursor_grain: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = start_cursor_override.py();
    if start_cursor_override.is_none() || end_cursor_override.is_none() {
        return Ok(None);
    }
    let (effective, grain) = cursor_typing(cursor_type, cursor_grain)?;
    let (Some(start), Some(end)) = (
        try_parse(start_cursor_override, &effective)?,
        try_parse(end_cursor_override, &effective)?,
    ) else {
        return Ok(None);
    };
    new_record_with(
        py,
        RecordId::CursorBounds,
        &[("start", start), ("end", to_exclusive(&end, &grain)?)],
    )
    .map(Some)
}

fn parsed_bound(value: &Bound<'_, PyAny>) -> PyResult<Option<ParsedBound>> {
    let py: Python<'_> = value.py();
    let parsed: Bound<'_, PyAny> =
        match imported(py, DATETIME_MODULE, "datetime")?.call_method1("fromisoformat", (value,)) {
            Ok(parsed) => parsed,
            Err(error) if error.is_instance_of::<PyValueError>(py) => return Ok(None),
            Err(error) => return Err(error),
        };
    let mut midnight: bool = true;
    for part in ["hour", "minute", "second", "microsecond"] {
        midnight = midnight && native_field(&parsed, part)?.extract::<i64>()? == 0;
    }
    Ok(Some(ParsedBound {
        date: parsed
            .call_method0("date")?
            .call_method0("isoformat")?
            .extract()?,
        midnight,
    }))
}

/// A cursor bound for display, collapsing a whole-day midnight timestamp to a date.
#[pyfunction]
#[pyo3(signature = (*, value, cursor_type, cursor_grain))]
pub(crate) fn cursor_bound_display(
    value: &Bound<'_, PyAny>,
    cursor_type: &Bound<'_, PyAny>,
    cursor_grain: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let cursor_type: Option<String> = optional_text(cursor_type)?;
    let raw: String = value.extract()?;
    let parsed: Option<ParsedBound> = if cursor_type.as_deref() == Some(INTEGER_CURSOR) {
        None
    } else {
        parsed_bound(value)?
    };
    Ok(bound_display(
        &raw,
        (
            cursor_type.as_deref(),
            optional_text(cursor_grain)?.as_deref(),
        ),
        parsed.as_ref(),
    ))
}

/// The full materialization label used in plan summaries.
pub(crate) fn model_materialization_label(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(materialization_label(&label_facts(entry)?))
}

fn names(input_names: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    input_names
        .try_iter()?
        .map(|name| name?.extract())
        .collect()
}

/// Explain that a cursor window is empty because its inputs have no rows.
pub(crate) fn format_no_input_rows(input_names: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(no_input_rows(&names(input_names)?))
}

/// Explain that a cursor window waits on empty inputs while other inputs have rows.
#[pyfunction]
pub(crate) fn format_waiting_on_empty_inputs(input_names: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(waiting_on_empty_inputs(&names(input_names)?))
}

/// The explicit effective batch-size token resolved against the replay grain.
#[pyfunction]
#[pyo3(signature = (*, batch_size, effective_grain))]
pub(crate) fn resolve_effective_microbatch_batch_size(
    py: Python<'_>,
    batch_size: &str,
    effective_grain: &str,
) -> Result<String, PyErr> {
    effective_microbatch_batch_size(batch_size, effective_grain).map_err(|error| {
        record_class(py, RecordId::CursorGrain)
            .and_then(|grain| grain.call1((error.0,)))
            .err()
            .unwrap_or_else(|| PyValueError::new_err("unknown cursor grain"))
    })
}

fn as_datetime<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let inner: Bound<'py, PyAny> = native_field(value, "value")?;
    if value.is_instance(&record_class(py, RecordId::TimestampValue)?)? {
        return Ok(inner);
    }
    let datetime: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let midnight: Bound<'py, PyAny> = native_field(&datetime, "min")?.call_method0("time")?;
    datetime.call_method1("combine", (inner, midnight))
}

fn presume_utc<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if !native_field(&value, "tzinfo")?.is_none() {
        return Ok(value);
    }
    let options: Bound<'py, PyDict> = PyDict::new(py);
    pyo3::types::PyDictMethods::set_item(
        &options,
        "tzinfo",
        imported(py, DATETIME_MODULE, "UTC")?,
    )?;
    value.call_method("replace", (), Some(&options))
}

/// Comparable bounds, presuming a naive side UTC when the other is aware.
#[pyfunction]
#[pyo3(signature = (*, start, end))]
pub(crate) fn aligned_timestamp_bounds<'py>(
    start: &Bound<'py, PyAny>,
    end: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = start.py();
    let mut start_at: Bound<'py, PyAny> = as_datetime(start)?;
    let mut end_at: Bound<'py, PyAny> = as_datetime(end)?;
    if native_field(&start_at, "tzinfo")?.is_none() != native_field(&end_at, "tzinfo")?.is_none() {
        start_at = presume_utc(start_at)?;
        end_at = presume_utc(end_at)?;
    }
    PyTuple::new(py, [start_at, end_at])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inclusive_cursor_end, module)?)?;
    module.add_function(wrap_pyfunction!(cursor_bound_display, module)?)?;
    module.add_function(wrap_pyfunction!(format_waiting_on_empty_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_effective_microbatch_batch_size,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(aligned_timestamp_bounds, module)?)?;
    Ok(())
}
