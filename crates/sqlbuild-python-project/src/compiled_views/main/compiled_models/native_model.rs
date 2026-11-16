//! The native view behind a compiled model object.

use crate::compiled_views::models::NativeCompiledModel;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

/// The native view behind a model object; any other object is a Python-built record, counted.
#[inline]
pub fn native_model<'a>(model: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a NativeCompiledModel>> {
    if let Ok(view) = model.cast::<NativeCompiledModel>() {
        return Ok(Some(view.get()));
    }
    count_stage(model.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
