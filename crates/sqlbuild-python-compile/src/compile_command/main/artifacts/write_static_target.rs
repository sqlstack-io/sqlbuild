//! Write offline compiled output under `target_dir`, reusing SQL tests planned ahead.

use crate::compile_command::_helpers::artifacts::target_writer;
use crate::compile_command::models::Prepared;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Write offline compiled output under `target_dir`, reusing SQL tests planned ahead.
#[inline]
pub fn write_static_target<'py>(
    target_dir: &Bound<'py, PyAny>,
    (adapter, project): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    manifest: &Bound<'py, PyAny>,
    prepared: Prepared<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    target_writer::write_static_target(target_dir, (adapter, project), manifest, prepared)
}
