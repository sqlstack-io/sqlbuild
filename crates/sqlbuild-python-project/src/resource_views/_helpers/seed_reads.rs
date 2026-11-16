//! A compiled seed's attributes the planner reads, from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::resource_views::models::NativeCompiledSeed;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

/// The native view behind a seed object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_seed<'a>(
    seed: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledSeed>> {
    if let Ok(view) = seed.cast::<NativeCompiledSeed>() {
        return Ok(Some(view.get()));
    }
    count_stage(seed.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
