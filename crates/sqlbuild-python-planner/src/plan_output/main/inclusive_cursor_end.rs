//! The last cursor value included by an exclusive end bound.

use crate::plan_output::_helpers::display::cursor_typing;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_front::cursor::main::scalars::render_value::render_value;
use sqlbuild_python_front::cursor::main::scalars::to_inclusive::to_inclusive;
use sqlbuild_python_front::cursor::main::scalars::try_parse::try_parse;

/// The last cursor value included by an exclusive end bound.
#[pyfunction]
#[pyo3(signature = (*, end, cursor_type, cursor_grain))]
pub fn inclusive_cursor_end(
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
