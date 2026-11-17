//! Public records of semantic checks.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// The semantic completion stage's inputs.
#[derive(Debug)]
pub struct SemanticCompletion<'py> {
    pub project: Bound<'py, PyAny>,
    pub profile: Bound<'py, PyAny>,
    pub binding_results: Bound<'py, PyAny>,
    pub resource_sql_analysis: bool,
    pub session: Option<Bound<'py, PyAny>>,
}
