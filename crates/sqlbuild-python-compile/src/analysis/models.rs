//! Public records of analysis.

use sqlbuild_analysis::binding_locations::models::AuthoredSpan;
use sqlbuild_python_project::analysis::models::Positional;

/// One located diagnostic as Python reads it, its span `(line, column, end line, end column)`.
#[derive(Debug)]
pub struct LocatedRow {
    pub code: String,
    pub message: String,
    pub severity: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub span: Option<Positional<AuthoredSpan>>,
}

impl<'py> pyo3::IntoPyObject<'py> for LocatedRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.code,
                self.message,
                self.severity,
                self.line,
                self.column,
                self.span,
            ),
            py,
        )
    }
}
