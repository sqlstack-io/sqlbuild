//! Render one validated constant with the active adapter's typed-value contract.

use crate::render::_helpers::declarations::declaration_values::rendered_constant;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::NativeAttr;

/// Render one validated constant with the active adapter's typed-value contract.
pub fn constant_declaration_sql<'py>(
    declaration: &Bound<'py, PyAny>,
    value_renderer: &Bound<'py, PyAny>,
    (collection_rendering, file_path): (&Bound<'py, PyAny>, Option<&str>),
) -> PyResult<String> {
    let py: Python<'py> = declaration.py();
    let render_as: Bound<'py, PyAny> = native_field(declaration, "render_as")?;
    let selected: Bound<'py, PyAny> = if render_as.is_truthy()? {
        render_as
    } else {
        collection_rendering.clone()
    };
    let error: PyErr = match rendered_constant(declaration, value_renderer, &selected) {
        Ok(rendered) => return Ok(rendered),
        Err(error) => error,
    };
    let rendering_error: Bound<'py, PyAny> = host_entry(py, ExecutorHost::SqlValueRenderingError)?;
    let validation_error: Bound<'py, PyAny> = native_attr(py, NativeAttr::SqlValueValidationError)?;
    let raised: Bound<'py, PyAny> = error.value(py).clone().into_any();
    if !(raised.is_instance(&rendering_error)? || raised.is_instance(&validation_error)?) {
        return Err(error);
    }
    let relative_path: Bound<'py, PyAny> = native_field(declaration, "relative_path")?;
    let location: String = match file_path {
        Some(file_path) => file_path.to_owned(),
        None => relative_path.str()?.to_string(),
    };
    let compile_error: Bound<'py, PyAny> =
        error_class(py, ErrorId::CompileInput)?.call1((format!(
            "{} constant '{}' could not be rendered in '{}' by adapter '{}' as {}: {}",
            relative_path.str()?,
            native_field(declaration, "name")?.str()?,
            location,
            native_field(value_renderer, "adapter_name")?.str()?,
            native_field(&selected, "value")?.str()?,
            raised.str()?,
        ),))?;
    compile_error.setattr("__cause__", &raised)?;
    Err(PyErr::from_value(compile_error))
}
