//! Public traits and type aliases of render.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_sqltext::compiler::models::CharSpan;

/// One substitution span: source start and end, output start and end.
pub type SpanRow = CharSpan;

/// The declaration contexts of macro files in a scoped compile.
pub trait MacroScopes<'py> {
    /// The context the file at `path` sees, or `None` when the compile is unscoped.
    fn file_context(&self, path: &str) -> PyResult<Option<Bound<'py, PyAny>>>;
}
