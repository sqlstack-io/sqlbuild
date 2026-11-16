//! Resolve typed cursor overrides to generic start/end for one model.

use crate::planner_resolution::_helpers::cursor::cursor_ranges::INTEGER;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::TIMESTAMP;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::compile_input_error;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::config_bound;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::order;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::token;
use crate::planner_resolution::main::cursor::config_str::config_str;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::cursor::models::CursorSession;
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use std::cmp::Ordering;

/// Resolve typed cursor overrides to generic start/end for one model.
pub fn model_cursor_overrides<'py>(
    model: &Bound<'py, PyAny>,
    cursor_overrides: &Bound<'py, PyAny>,
    (start, end): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = model.py();
    let cursor_type = config_str(model, "cursor_type")?;
    let (mut resolved_start, mut resolved_end) = (start, end);
    let typed_fields = if cursor_overrides.is_none() {
        None
    } else if cursor_type.eq(TIMESTAMP)? {
        Some(("start_ts", "end_ts"))
    } else if cursor_type.eq(INTEGER)? {
        Some(("start_int", "end_int"))
    } else {
        None
    };
    if let Some((start_field, end_field)) = typed_fields {
        let typed_start = native_field(cursor_overrides, start_field)?;
        if !typed_start.is_none() {
            resolved_start = token(py, &typed_start)?;
        }
        let typed_end = native_field(cursor_overrides, end_field)?;
        if !typed_end.is_none() {
            resolved_end = token(py, &typed_end)?;
        }
    }
    let session = CursorSession::new(py)?;
    let name = name_object(model)?.str()?;
    let cursor_start = config_bound(model, "cursor_start")?;
    let cursor_end = config_bound(model, "cursor_end")?;
    if !resolved_start.is_none()
        && !cursor_start.is_none()
        && order(&session, (&resolved_start, &cursor_start), &cursor_type)? == Ordering::Less
    {
        return Err(compile_input_error(
            py,
            format!(
                "model '{name}': explicit start {} is before cursor_start {}",
                resolved_start.str()?,
                cursor_start.str()?
            ),
        ));
    }
    if !cursor_end.is_none() {
        for (value, label) in [(&resolved_start, "start"), (&resolved_end, "end")] {
            if !value.is_none()
                && order(&session, (value, &cursor_end), &cursor_type)? != Ordering::Less
            {
                return Err(compile_input_error(
                    py,
                    format!(
                        "model '{name}': explicit {label} {} is at or beyond exclusive cursor_end {}",
                        value.str()?,
                        cursor_end.str()?
                    ),
                ));
            }
        }
    }
    Ok((resolved_start, resolved_end))
}
