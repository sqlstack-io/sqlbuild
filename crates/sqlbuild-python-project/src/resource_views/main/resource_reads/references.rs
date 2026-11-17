//! A resource's `references` from whichever native view it is; any other object is read by attribute, counted.

use crate::compiled_views::models::NativeCompiledModel;
use crate::resource_views::models::NativeCompiledFunction;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A resource's `references` from whichever native view it is; any other object is read by attribute, counted.
pub fn references<'py>(resource: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = resource.py();
    if let Ok(view) = resource.cast::<NativeCompiledModel>() {
        let view: &NativeCompiledModel = view.get();
        return view
            .references(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    if let Ok(view) = resource.cast::<NativeCompiledFunction>() {
        let view: &NativeCompiledFunction = view.get();
        return view
            .references(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    count_stage(py, COMPILED_VIEW_FALLBACKS, 1)?;
    native_field(resource, "references")
}
