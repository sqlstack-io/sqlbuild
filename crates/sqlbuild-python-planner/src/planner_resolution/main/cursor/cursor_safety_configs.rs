//! The future-end and automatic-start policies effective for one model.

use crate::planner_entries::models::ModelPlanContext;
use crate::planner_phases::main::inputs::class_field::class_field;
use crate::planner_resolution::_helpers::cursor::cursor_policies::{
    future_cursor_config, start_cursor_config,
};
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The future-end and automatic-start policies effective for one model.
pub fn cursor_safety_configs<'py>(
    model: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    Ok((
        future_cursor_config(
            model,
            &class_field(context, |c: &ModelPlanContext| &c.future_cursor_config)?,
        )?,
        start_cursor_config(
            model,
            &class_field(context, |c: &ModelPlanContext| &c.start_cursor_config)?,
        )?,
    ))
}
