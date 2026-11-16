//! `build_effective_vars` for a discovered project and target, without CLI vars.

use crate::compile_attachment::main::build_effective_vars::build_effective_vars;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `build_effective_vars` for a discovered project and target, without CLI vars.
pub fn project_vars<'py>(
    discovered: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    build_effective_vars(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        target_config,
        PyDict::new(discovered.py()).as_any(),
    )
}
