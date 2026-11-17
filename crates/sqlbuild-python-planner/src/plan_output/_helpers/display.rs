//! Plan and progress display fragments over planner entries and cursor values.

use crate::plan_output::main::text::text;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_core::plan_display::constants::{
    DEFAULT_CURSOR_GRAIN, INTEGER_CURSOR, TIMESTAMP_CURSOR,
};
use sqlbuild_core::plan_display::main::format_no_input_rows::format_no_input_rows as no_input_rows;
use sqlbuild_core::plan_display::main::format_waiting_on_empty_inputs::format_waiting_on_empty_inputs as waiting_on_empty_inputs;
use sqlbuild_core::plan_display::main::model_materialization_label::model_materialization_label as materialization_label;
use sqlbuild_core::plan_display::models::{ModelLabelFacts, ParsedBound};

use crate::plan_output::_helpers::values::optional_text;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const DATETIME_MODULE: &str = "datetime";

pub(crate) fn label_facts(entry: &Bound<'_, PyAny>) -> PyResult<ModelLabelFacts> {
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

/// The cursor type, defaulting to timestamp, and its grain object (`None` for integers).
pub(crate) fn cursor_typing<'py>(
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

pub(crate) fn parsed_bound(value: &Bound<'_, PyAny>) -> PyResult<Option<ParsedBound>> {
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

pub(crate) fn as_datetime<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let inner: Bound<'py, PyAny> = native_field(value, "value")?;
    if value.is_instance(&record_class(py, RecordId::TimestampValue)?)? {
        return Ok(inner);
    }
    let datetime: Bound<'py, PyAny> = imported(py, DATETIME_MODULE, "datetime")?;
    let midnight: Bound<'py, PyAny> = native_field(&datetime, "min")?.call_method0("time")?;
    datetime.call_method1("combine", (inner, midnight))
}

pub(crate) fn presume_utc<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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
