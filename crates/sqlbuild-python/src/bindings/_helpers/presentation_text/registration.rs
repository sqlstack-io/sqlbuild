//! Register the shared CLI text helpers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::presentation_text::main::register_document::register_document;
use sqlbuild_python_base::presentation_text::main::register_functions::register_functions;
use sqlbuild_python_base::presentation_text::main::register_style::register_style;
use sqlbuild_python_base::presentation_text::main::register_terminal::register_terminal;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_document(module)?;
    register_functions(module)?;
    register_style(module)?;
    register_terminal(module)
}
