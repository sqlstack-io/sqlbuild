//! The compiled project's database and schema for Python nodes outside any model.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::main::setup::model_free_locations::model_free_locations;

/// The compiled project's `model_free_locations`, falling back to the adapter's defaults.
pub fn project_model_free_locations<'py>(
    project: &Bound<'py, PyAny>,
    loader_schema: Option<&str>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    model_free_locations(
        (
            native_field(project, "effective_target_database")?,
            native_field(project, "effective_target_schema")?,
        ),
        loader_schema,
        |method| adapter.call_method0(method),
    )
}
