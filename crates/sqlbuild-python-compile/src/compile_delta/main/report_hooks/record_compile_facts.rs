//! Record the compiled models with their references, and the report's diagnostics.

use crate::compile_delta::_helpers::report_hooks::record_compile_facts_inner;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::compile_context::main::delta::guarded::guarded;

/// Record the compiled models with their references, and the report's diagnostics.
pub fn record_compile_facts(
    graph: &Bound<'_, PyAny>,
    diagnostics: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    guarded(|| record_compile_facts_inner(graph, diagnostics))
}
