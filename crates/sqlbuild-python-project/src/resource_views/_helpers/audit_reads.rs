//! A compiled audit's attributes the planner reads, from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::resource_views::models::NativeCompiledAudit;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;

/// The native view behind an audit object; any other object is a Python-built record, counted.
#[inline]
pub(crate) fn native_audit<'a>(
    audit: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledAudit>> {
    if let Ok(view) = audit.cast::<NativeCompiledAudit>() {
        return Ok(Some(view.get()));
    }
    count_stage(audit.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}
