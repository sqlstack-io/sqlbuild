//! The selected target name of a discovered project and its effective config, if any.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The selected target name of a discovered project and its effective config, if any.
#[inline]
pub fn target_selection<'py>(
    discovered: &Bound<'py, PyAny>,
    selected: Option<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    effective_config::target_selection(discovered, selected)
}
