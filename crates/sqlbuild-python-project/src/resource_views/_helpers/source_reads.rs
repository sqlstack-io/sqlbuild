//! A compiled source's attributes the planner reads, from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::resource_views::models::NativeCompiledSource;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

/// The native view behind a source object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_source<'a>(
    source: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledSource>> {
    if let Ok(view) = source.cast::<NativeCompiledSource>() {
        return Ok(Some(view.get()));
    }
    count_stage(source.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
