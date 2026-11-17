//! One planned model's requested, resolved and deferred cursor state, for text and JSON plans.

use crate::plan_output::main::aligned_timestamp_bounds::aligned_timestamp_bounds;
use crate::plan_output::main::resolve_effective_microbatch_batch_size::resolve_effective_microbatch_batch_size;
use crate::plan_output::main::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::plan_output::_helpers::values::optional_text;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::constants::GRAIN_NAMES;

const MICROBATCH_MODE: &str = "microbatch";
const TIMESTAMP_CURSOR: &str = "timestamp";
const INTEGER_CURSOR: &str = "integer";
const WATERMARK_STRATEGY: &str = "watermark";
const SECOND_GRAIN: &str = "second";

/// `CursorBoundsOwner` and `CursorResolutionStatus` of one model, as their values.
pub(crate) const RUNTIME_OWNER: &str = "runtime";
pub(crate) const PLANNER_OWNER: &str = "planner";
pub(crate) const NO_INPUT_ROWS: &str = "no_input_rows";
pub(crate) const RESOLVED: &str = "resolved";
pub(crate) const DEFERRED: &str = "deferred";
pub(crate) const UNAVAILABLE: &str = "unavailable";

/// `CursorPlanDetails` of one model entry.
pub(crate) struct CursorDetails<'py> {
    pub(crate) requested_start: Option<String>,
    pub(crate) requested_end: Option<String>,
    pub(crate) runtime_owned: bool,
    pub(crate) resolution_status: &'static str,
    /// Both bounds are scalars, not sentinels.
    pub(crate) resolved_bounds: Option<Bound<'py, PyAny>>,
    pub(crate) declared_grain: Option<String>,
    pub(crate) effective_grain: Option<String>,
    pub(crate) declared_batch_size: Option<String>,
    pub(crate) effective_batch_size: Option<String>,
    pub(crate) planned_batch_count: Option<i64>,
    pub(crate) empty_inputs: Bound<'py, PyAny>,
    pub(crate) waiting_on_empty_inputs: bool,
}

fn grain_rank(grain: &str) -> PyResult<usize> {
    GRAIN_NAMES
        .iter()
        .position(|(known, _)| *known == grain)
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("'{grain}' is not a valid CursorGrain"))
        })
}

/// The coarsest timestamp grain declared across a model and its inputs.
fn effective_grain(entry: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let declared: Option<String> = optional_text(&native_field(entry, "cursor_grain")?)?;
    if !native_field(entry, "cursor_type")?.eq(TIMESTAMP_CURSOR)? {
        return Ok(declared);
    }
    let mut effective: String = declared.unwrap_or_else(|| SECOND_GRAIN.to_owned());
    if native_field(entry, "microbatch_strategy")?.eq(WATERMARK_STRATEGY)? {
        return Ok(Some(effective));
    }
    for relation in native_field(entry, "cursor_input_relations")?.try_iter()? {
        let relation_grain: String = optional_text(&native_field(&relation?, "cursor_grain")?)?
            .unwrap_or_else(|| SECOND_GRAIN.to_owned());
        if grain_rank(&relation_grain)? > grain_rank(&effective)? {
            effective = relation_grain;
        }
    }
    Ok(Some(effective))
}

/// How many windows the executor will create over timestamp or integer bounds.
fn count_batches(
    bounds: &Bound<'_, PyAny>,
    batch_size: &str,
    cursor_type: &str,
) -> PyResult<Option<i64>> {
    let py: Python<'_> = bounds.py();
    let (start, end) = (native_field(bounds, "start")?, native_field(bounds, "end")?);
    if cursor_type == TIMESTAMP_CURSOR {
        let duration: Bound<'_, PyAny> =
            record_class(py, RecordId::Duration)?.call_method1("parse", (batch_size,))?;
        if duration.is_none() {
            return Ok(None);
        }
        let temporal = |value: &Bound<'_, PyAny>| -> PyResult<bool> {
            Ok(value.is_instance(&record_class(py, RecordId::DateValue)?)?
                || value.is_instance(&record_class(py, RecordId::TimestampValue)?)?)
        };
        if !temporal(&start)? || !temporal(&end)? {
            return Ok(None);
        }
        let (mut current, end): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
            aligned_timestamp_bounds(&start, &end)?.extract()?;
        let minimum: Bound<'_, PyAny> = py.import("builtins")?.getattr("min")?;
        let mut count: i64 = 0;
        while current.lt(&end)? {
            current = minimum.call1((duration.call_method1("add_to", (&current,))?, &end))?;
            count += 1;
        }
        return Ok(Some(count));
    }
    if cursor_type != INTEGER_CURSOR {
        return Ok(None);
    }
    let integer: Bound<'_, PyAny> = record_class(py, RecordId::IntegerValue)?;
    if !start.is_instance(&integer)? || !end.is_instance(&integer)? {
        return Ok(None);
    }
    let (start, end): (i128, i128) = (
        native_field(&start, "value")?.extract()?,
        native_field(&end, "value")?.extract()?,
    );
    let Ok(size) = batch_size.trim().replace('_', "").parse::<i128>() else {
        return Ok(None);
    };
    if size <= 0 || start >= end {
        return Ok(Some(0));
    }
    Ok(Some(
        i64::try_from((end - start + size - 1) / size).unwrap_or(i64::MAX),
    ))
}

/// `build_cursor_plan_details`: the model's cursor details, or `None` without a cursor column.
pub(crate) fn cursor_details<'py>(
    entry: &Bound<'py, PyAny>,
) -> PyResult<Option<CursorDetails<'py>>> {
    let py: Python<'py> = entry.py();
    if native_field(entry, "cursor_column")?.is_none() {
        return Ok(None);
    }
    let requested_start: Option<String> =
        optional_text(&native_field(entry, "start_cursor_override")?)?;
    let requested_end: Option<String> =
        optional_text(&native_field(entry, "end_cursor_override")?)?;
    let mut any_runtime: bool = false;
    for relation in native_field(entry, "cursor_input_relations")?.try_iter()? {
        any_runtime = any_runtime || native_field(&relation?, "is_runtime_owned")?.is_truthy()?;
    }
    let runtime_owned: bool =
        any_runtime && !(requested_start.is_some() && requested_end.is_some());
    let microbatch_range: Bound<'py, PyAny> = native_field(entry, "microbatch_range")?;
    let mut resolved_bounds: Option<Bound<'py, PyAny>> = if microbatch_range.is_truthy()? {
        Some(microbatch_range)
    } else {
        let bounds: Bound<'py, PyAny> = native_field(entry, "cursor_bounds")?;
        (!bounds.is_none()).then_some(bounds)
    };
    let sentinel: Bound<'py, PyAny> = host_entry(py, ExecutorHost::BoundSentinel)?;
    if let Some(bounds) = &resolved_bounds
        && (native_field(bounds, "start")?.is_instance(&sentinel)?
            || native_field(bounds, "end")?.is_instance(&sentinel)?)
    {
        resolved_bounds = None;
    }
    let effective_grain: Option<String> = effective_grain(entry)?;
    let declared_batch_size: Option<String> = optional_text(&native_field(entry, "batch_size")?)?;
    let effective_batch_size: Option<String> = match (&declared_batch_size, &effective_grain) {
        (Some(batch_size), Some(grain)) => Some(resolve_effective_microbatch_batch_size(
            py, batch_size, grain,
        )?),
        (batch_size, _) => batch_size.clone(),
    };
    let cursor_type: Option<String> = optional_text(&native_field(entry, "cursor_type")?)?;
    let planned_batch_count: Option<i64> =
        match (&resolved_bounds, &effective_batch_size, &cursor_type) {
            (Some(bounds), Some(batch_size), Some(cursor_type))
                if native_field(entry, "incremental_mode")?.eq(MICROBATCH_MODE)? =>
            {
                count_batches(bounds, batch_size, cursor_type)?
            }
            _ => None,
        };
    let empty_inputs: Bound<'py, PyAny> = native_field(entry, "empty_cursor_inputs")?;
    let resolution_status: &'static str = if empty_inputs.is_truthy()? {
        NO_INPUT_ROWS
    } else if resolved_bounds.is_some() {
        RESOLVED
    } else if runtime_owned {
        DEFERRED
    } else {
        UNAVAILABLE
    };
    Ok(Some(CursorDetails {
        requested_start,
        requested_end,
        runtime_owned,
        resolution_status,
        resolved_bounds,
        declared_grain: optional_text(&native_field(entry, "cursor_grain")?)?,
        effective_grain,
        declared_batch_size,
        effective_batch_size,
        planned_batch_count,
        empty_inputs,
        waiting_on_empty_inputs: native_field(entry, "waiting_on_empty_inputs")?.is_truthy()?,
    }))
}

/// `runtime_cursor_inputs_clause`: the runtime-owned cursor inputs this run brings up to date.
pub(crate) fn runtime_inputs_clause(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut labels: Vec<String> = Vec::new();
    for relation in native_field(entry, "cursor_input_relations")?.try_iter()? {
        let relation: Bound<'_, PyAny> = relation?;
        if !native_field(&relation, "is_runtime_owned")?.is_truthy()? {
            continue;
        }
        let input_name: Bound<'_, PyAny> = native_field(&relation, "input_name")?;
        let name: String = if input_name.is_truthy()? {
            text(&input_name)?
        } else {
            text(&native_field(&relation, "relation")?)?
        };
        let kind: Bound<'_, PyAny> = native_field(&relation, "input_kind")?;
        let label: String = if kind.is_none() {
            name
        } else {
            format!("{} {name}", text(&native_field(&kind, "value")?)?)
        };
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(match labels.as_slice() {
        [] => "once its inputs are up to date".to_owned(),
        [only] => format!("once {only} is up to date"),
        [first @ .., last] => format!("once {} and {last} are up to date", first.join(", ")),
    })
}

pub(crate) fn owner(details: &CursorDetails<'_>) -> &'static str {
    if details.runtime_owned {
        RUNTIME_OWNER
    } else {
        PLANNER_OWNER
    }
}
