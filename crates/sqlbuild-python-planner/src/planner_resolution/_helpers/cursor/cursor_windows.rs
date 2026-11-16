//! Empty-window decisions and snapshot grain conversion.

use crate::planner_resolution::_helpers::references::values::string;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_cursor::algebra::main::floor_to_grain::floor_to_grain;
use sqlbuild_cursor::planner::main::empty_inputs::empty_inputs;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::scalars::parse_value::parse_value;
use sqlbuild_python_front::cursor::models::CursorSession;

#[pyfunction]
#[pyo3(signature = (*, empty_input_names, waits_for_every_input, inputs_with_rows, has_window_start))]
pub(crate) fn decide_empty_cursor_inputs<'py>(
    empty_input_names: &Bound<'py, PyAny>,
    waits_for_every_input: bool,
    inputs_with_rows: bool,
    has_window_start: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = empty_input_names.py();
    if !empty_input_names.is_truthy()?
        || !empty_inputs(waits_for_every_input, inputs_with_rows, has_window_start)
    {
        return new_record(py, RecordId::EmptyCursorInputDecision, (), None);
    }
    new_record_with(
        py,
        RecordId::EmptyCursorInputDecision,
        &[
            ("input_names", empty_input_names.clone()),
            (
                "waiting_on_empty_inputs",
                inputs_with_rows.into_pyobject(py)?.to_owned().into_any(),
            ),
        ],
    )
}

pub(crate) fn cursor_inputs_without_rows<'py>(
    cursor_snapshot: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if native_field(cursor_snapshot, "unreadable_input_names")?.is_truthy()? {
        return new_record(
            cursor_snapshot.py(),
            RecordId::EmptyCursorInputDecision,
            (),
            None,
        );
    }
    let rows = native_field(cursor_snapshot, "upstream_maxes")?.is_truthy()?
        || native_field(cursor_snapshot, "upstream_terminal_ends")?.is_truthy()?
        || native_field(cursor_snapshot, "upstream_availability_ends")?.is_truthy()?;
    let start = !native_field(cursor_snapshot, "target_max")?.is_none()
        || native_field(cursor_snapshot, "upstream_mins")?.is_truthy()?
        || native_field(cursor_snapshot, "upstream_terminal_starts")?.is_truthy()?;
    decide_empty_cursor_inputs(
        &native_field(cursor_snapshot, "empty_input_names")?,
        !native_field(cursor_snapshot, "cursor_watermark_mode")?.eq("any")?,
        rows,
        start,
    )
}

#[pyfunction]
#[pyo3(signature = (*, cursor_type, cursor_start))]
pub(crate) fn empty_cursor_window<'py>(
    cursor_type: &Bound<'py, PyAny>,
    cursor_start: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cursor_type.py();
    let kind = if cursor_type.is_truthy()? {
        cursor_type.clone()
    } else {
        string(py, "timestamp")
    };
    let raw = if !cursor_start.is_none() {
        cursor_start.clone()
    } else if kind.eq("integer")? {
        string(py, "0")
    } else {
        string(py, "1970-01-01T00:00:00")
    };
    let mut bound = parse_value(&raw, &kind)?;
    if cursor_start.is_none() && !kind.eq("integer")? {
        bound = new_record_with(
            py,
            RecordId::TimestampValue,
            &[("value", native_field(&bound, "value")?)],
        )?;
    }
    new_record_with(
        py,
        RecordId::CursorBounds,
        &[("start", bound.clone()), ("end", bound)],
    )
}

#[pyfunction]
#[pyo3(signature = (*, cursor_type))]
pub(crate) fn cursor_window_override_flags(cursor_type: Option<&str>) -> String {
    let suffix = if cursor_type == Some("integer") {
        "int"
    } else {
        "ts"
    };
    format!("--start-cursor-{suffix} and --end-cursor-{suffix}")
}

#[pyfunction]
#[pyo3(signature = (*, model_name, input_names, cursor_type))]
pub(crate) fn empty_input_rebuild_refusal(
    model_name: &str,
    input_names: Vec<String>,
    cursor_type: Option<&str>,
) -> (String, String) {
    (
        format!(
            "model '{model_name}' was not rebuilt: its cursor inputs have no rows ({}), so rebuilding it now would leave it empty",
            input_names.join(", ")
        ),
        format!(
            "load the inputs first, or set the window explicitly with {}",
            cursor_window_override_flags(cursor_type)
        ),
    )
}

pub(crate) fn unreadable_cursor_inputs_error<'py>(
    py: Python<'py>,
    model_name: &str,
    cursor_type: Option<&str>,
    input_names: Vec<String>,
) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &error_class(py, ErrorId::PlannerInput)?,
        &[
            (
                "message",
                string(
                    py,
                    &format!(
                        "model '{model_name}' cannot work out its cursor window: cursor bounds could not be read from {}",
                        input_names.join(", ")
                    ),
                ),
            ),
            ("code", string(py, "S302")),
            (
                "help",
                string(
                    py,
                    &format!(
                        "check that each input exists and has its cursor column (`sqb debug` checks the connection), or set the window explicitly with {}",
                        cursor_window_override_flags(cursor_type)
                    ),
                ),
            ),
        ],
    )
}

pub(crate) fn without_destination_cursor<'py>(
    cursor_snapshot: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cursor_snapshot.py();
    let kwargs = PyDict::new(py);
    for name in ["target_max", "physical_target_max", "target_eligible_max"] {
        kwargs.set_item(name, py.None())?;
    }
    replaced_keywords(cursor_snapshot, &kwargs)
}

#[pyfunction]
#[pyo3(signature = (*, cursor_snapshot, cursor_type, effective_grain))]
pub(crate) fn normalize_cursor_snapshot_grain<'py>(
    cursor_snapshot: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    effective_grain: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !cursor_type.eq("timestamp")? || effective_grain.is_none() {
        return Ok(cursor_snapshot.clone());
    }
    let py = cursor_snapshot.py();
    let session = CursorSession::new(py)?;
    let grain = session.grain(effective_grain)?;
    let normalize = |value: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
        if value.is_none() {
            return Ok(value);
        }
        session.python_scalar(&floor_to_grain(&session.scalar(&value)?, grain))
    };
    let kwargs = PyDict::new(py);
    for key in ["target_max", "target_eligible_max"] {
        kwargs.set_item(key, normalize(native_field(cursor_snapshot, key)?)?)?;
    }
    for key in ["upstream_mins", "upstream_maxes"] {
        let values = cursor_snapshot
            .getattr(key)?
            .try_iter()?
            .map(|value| normalize(value?))
            .collect::<PyResult<Vec<_>>>()?;
        kwargs.set_item(key, PyTuple::new(py, values)?)?;
    }
    let physical = native_field(cursor_snapshot, "physical_target_max")?;
    kwargs.set_item(
        "physical_target_max",
        if physical.is_truthy()? {
            physical
        } else {
            native_field(cursor_snapshot, "target_max")?
        },
    )?;
    let mut ends: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
    for pair in native_field(cursor_snapshot, "upstream_end_inputs")?.try_iter()? {
        let pair = pair?;
        ends.push((normalize(pair.get_item(0)?)?, pair.get_item(1)?));
    }
    kwargs.set_item("upstream_end_inputs", PyTuple::new(py, ends)?)?;
    replaced_keywords(cursor_snapshot, &kwargs)
}
