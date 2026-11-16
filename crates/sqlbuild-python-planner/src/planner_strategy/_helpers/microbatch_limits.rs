//! Microbatch limits applied to planned model entries in CLI, project and model precedence.

use crate::planner_changes::main::native_enum::native_enum;
use crate::planner_strategy::main::values::input_error::input_error;
use crate::planner_strategy::main::values::text::text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;

use sqlbuild_analysis::planner_strategy::constants::MICROBATCH_LIMIT_WARNING_CODE;
use sqlbuild_analysis::planner_strategy::main::microbatch_limit_warning::microbatch_limit_warning;
use sqlbuild_core::plan_display::errors::UnknownCursorGrain;
use sqlbuild_core::plan_display::main::effective_timestamp_grain::effective_timestamp_grain;
use sqlbuild_core::plan_display::models::TimestampGrainFacts;

use crate::plan_output::main::resolve_effective_microbatch_batch_size::resolve_effective_microbatch_batch_size;
use crate::planner_strategy::_helpers::microbatches::{
    CAP_ACTIONS, capped_microbatch_bounds, count_microbatches,
};
use crate::planner_strategy::_helpers::values::optional_string;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::replace::replace;

const ERROR_ACTION: &str = "error";
const SKIP_ACTION: &str = "skip";
const CREATE_TABLE_ACTION: &str = "create_table";
const ROLLING_WINDOW_STRATEGY: &str = "rolling_window";
const FULL_REFRESH_REASON: &str = "full_refresh";

/// A planned entry and the warning its limit raised.
type LimitedEntry<'py> = (Bound<'py, PyAny>, Option<Bound<'py, PyAny>>);

/// `CursorGrain(grain)`'s error for a grain outside the known grains.
pub(crate) fn unknown_grain(py: Python<'_>, error: UnknownCursorGrain) -> PyErr {
    record_class(py, RecordId::CursorGrain)
        .and_then(|class| class.call1((error.0,)))
        .err()
        .unwrap_or_else(|| pyo3::exceptions::PyValueError::new_err("unknown cursor grain"))
}

/// The coarsest timestamp grain of a cursor and the `cursor_grain` of its input relations.
pub(crate) fn replay_grain(
    (cursor_type, microbatch_strategy, downstream_grain): (
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    relations: &Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let mut input_grains: Vec<Option<String>> = Vec::new();
    for relation in relations.try_iter()? {
        input_grains.push(optional_string(&native_field(&relation?, "cursor_grain")?)?);
    }
    effective_timestamp_grain(&TimestampGrainFacts {
        cursor_type,
        microbatch_strategy,
        downstream_grain,
        input_grains,
    })
    .map_err(|error| unknown_grain(relations.py(), error))
}

/// The entry's effective batch size and its batch count over the planned range.
fn batch_count(entry: &Bound<'_, PyAny>) -> PyResult<Option<(String, i128)>> {
    let py: Python<'_> = entry.py();
    let range: Bound<'_, PyAny> = native_field(entry, "microbatch_range")?;
    let batch_size: Bound<'_, PyAny> = native_field(entry, "batch_size")?;
    let cursor_type: Bound<'_, PyAny> = native_field(entry, "cursor_type")?;
    if range.is_none() || batch_size.is_none() || cursor_type.is_none() {
        return Ok(None);
    }
    let (batch_size, cursor_type): (String, String) = (text(&batch_size)?, text(&cursor_type)?);
    let grain: Option<String> = replay_grain(
        (
            Some(cursor_type.clone()),
            optional_string(&native_field(entry, "microbatch_strategy")?)?,
            optional_string(&native_field(entry, "cursor_grain")?)?,
        ),
        &native_field(entry, "cursor_input_relations")?,
    )?;
    let effective_size: String = match grain {
        Some(grain) => resolve_effective_microbatch_batch_size(py, &batch_size, &grain)?,
        None => batch_size,
    };
    let creates: bool = native_field(entry, "action")?.eq(CREATE_TABLE_ACTION)?;
    let count: i128 = count_microbatches(&range, &effective_size, &cursor_type, creates)?;
    Ok(Some((effective_size, count)))
}

/// The entry with one batch limit applied, and the warning when a non-error limit is exceeded.
pub(crate) fn limited<'py>(
    entry: &Bound<'py, PyAny>,
    max_batches: Bound<'py, PyAny>,
    action: Bound<'py, PyAny>,
) -> PyResult<LimitedEntry<'py>> {
    let py: Python<'py> = entry.py();
    if native_field(entry, "action")?.eq(SKIP_ACTION)? {
        return Ok((entry.clone(), None));
    }
    let action_text: String = text(&action)?;
    let maximum: Option<i128> = if max_batches.is_none() {
        None
    } else {
        Some(max_batches.extract()?)
    };
    let counted: Option<(String, i128)> = batch_count(entry)?;
    let name: Bound<'py, PyAny> = native_field(entry, "name")?;
    let cap: bool = CAP_ACTIONS.contains(&action_text.as_str());
    let warning: Option<String> = match &counted {
        Some((_, count)) => {
            microbatch_limit_warning(&text(&name)?, (maximum, *count), (&action_text, cap))
        }
        None => None,
    };
    let mut range: Bound<'py, PyAny> = native_field(entry, "microbatch_range")?;
    if let (Some(_), Some((size, _)), Some(maximum), true) =
        (&warning, &counted, maximum, cap && !range.is_none())
    {
        let cursor_type: String =
            optional_string(&native_field(entry, "cursor_type")?)?.unwrap_or_default();
        range = capped_microbatch_bounds(range, (size, &cursor_type), maximum, &action_text)?;
    }
    let none: Bound<'py, PyAny> = py.None().into_bound(py);
    let limited_entry: Bound<'py, PyAny> = replace(
        entry,
        &[
            ("microbatch_range", range),
            (
                "microbatch_limit_count",
                counted
                    .as_ref()
                    .map(|(_, count)| *count)
                    .into_pyobject(py)?
                    .into_any(),
            ),
            (
                "microbatch_limit_action",
                if max_batches.is_none() { none } else { action },
            ),
            ("microbatch_limit", max_batches),
            (
                "microbatch_limit_warning",
                warning.clone().into_pyobject(py)?.into_any(),
            ),
        ],
    )?;
    let Some(message) = warning else {
        return Ok((limited_entry, None));
    };
    if action_text == ERROR_ACTION {
        return Err(input_error(py, ErrorId::CompileInput, &message, None));
    }
    let plan_warning: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::PlanWarning,
        &[
            ("model_name", name),
            (
                "severity",
                native_enum(py, RecordId::WarningSeverity, "warning")?,
            ),
            (
                "code",
                PyString::new(py, MICROBATCH_LIMIT_WARNING_CODE).into_any(),
            ),
            ("message", PyString::new(py, &message).into_any()),
        ],
    )?;
    Ok((limited_entry, Some(plan_warning)))
}

/// Apply one batch limit to a planned microbatch entry, failing when an `error` limit is exceeded.
#[pyfunction]
#[pyo3(signature = (*, entry, max_batches, action))]
pub(crate) fn apply_microbatch_limit<'py>(
    entry: &Bound<'py, PyAny>,
    max_batches: Bound<'py, PyAny>,
    action: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = entry.py();
    let (limited_entry, warning) = limited(entry, max_batches, action)?;
    PyTuple::new(
        py,
        [
            limited_entry,
            warning.unwrap_or_else(|| py.None().into_bound(py)),
        ],
    )
}

/// `(entry, warnings)` with the limit outcome's warning, if any.
pub(crate) fn with_warnings<'py>(
    (entry, warning): LimitedEntry<'py>,
    mut warnings: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = entry.py();
    warnings.extend(warning);
    PyTuple::new(py, [entry, PyTuple::new(py, warnings)?.into_any()])
}

/// The default limit: none for an incremental rolling window, else the model's limit or the configured ceiling.
pub(crate) fn default_limit<'py>(
    entry: &Bound<'py, PyAny>,
    configured: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = entry.py();
    if native_field(entry, "microbatch_strategy")?.eq(ROLLING_WINDOW_STRATEGY)?
        && !native_field(entry, "reason")?.eq(FULL_REFRESH_REASON)?
        && native_field(entry, "start_cursor_override")?.is_none()
        && native_field(entry, "end_cursor_override")?.is_none()
    {
        return Ok(py.None().into_bound(py));
    }
    let model_limit: Bound<'py, PyAny> = native_field(entry, "microbatch_limit")?;
    Ok(if model_limit.is_truthy()? {
        model_limit
    } else {
        configured
    })
}
