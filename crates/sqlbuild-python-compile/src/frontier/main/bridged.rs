//! `stage()` run inside the macro bridge user macros render through.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// `stage()` run inside the macro bridge user macros render through.
pub fn bridged<'py>(stage: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(stage.py(), UserCodeHost::RunWithMacroBridge)?,
        &[("stage", stage.clone())],
    )
}
