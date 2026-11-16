//! A compiled function's attributes the planner reads, from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::resource_views::models::NativeCompiledFunction;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

/// The native view behind a function object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_function<'a>(
    function: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledFunction>> {
    if let Ok(view) = function.cast::<NativeCompiledFunction>() {
        return Ok(Some(view.get()));
    }
    count_stage(function.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
