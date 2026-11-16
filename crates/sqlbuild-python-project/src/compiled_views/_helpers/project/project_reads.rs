//! A compiled project's resource collections, read from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::compiled_views::models::NativeProjectView;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

pub(crate) const MODELS_FIELD: &str = "models";
pub(crate) const SOURCES_FIELD: &str = "sources";
pub(crate) const SEEDS_FIELD: &str = "seeds";
pub(crate) const FUNCTIONS_FIELD: &str = "functions";
pub(crate) const AUDITS_FIELD: &str = "audits";

/// The native view behind a project object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_project<'a>(
    project: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeProjectView>> {
    if let Ok(view) = project.cast::<NativeProjectView>() {
        return Ok(Some(view.get()));
    }
    count_stage(project.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
