//! Semantic styling for human CLI output, as a Python class.

use crate::presentation_text::models::PythonCliStyle;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PythonCliStyle>()
}
