//! Register the native compile attachment functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::attachments::main::register_ceremonial_select::register_ceremonial_select;
use sqlbuild_python_front::attachments::main::register_cursor_intrinsics::register_cursor_intrinsics;
use sqlbuild_python_front::attachments::main::register_reference_markers::register_reference_markers;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_cursor_intrinsics(module)?;
    register_ceremonial_select(module)?;
    register_reference_markers(module)?;
    Ok(())
}
