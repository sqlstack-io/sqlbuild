//! The adapter behaviour compile inputs need, from the discovered project and its adapter.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::resolve_effective_collection_rendering::resolve_effective_collection_rendering;

/// The adapter behaviour compile inputs need, from the discovered project and its adapter.
pub fn compile_adapter_context<'py>(
    discovered: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let collection_rendering: Bound<'py, PyAny> = resolve_effective_collection_rendering(
        &native_field(discovered, "project_config")?,
        &py.None().into_bound(py),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    new_record_with(
        py,
        RecordId::CompileAdapterContext,
        &[
            ("value_renderer", adapter.clone()),
            ("collection_rendering", collection_rendering),
            (
                "python_functions_inherit_default_namespace",
                adapter.call_method0("python_functions_inherit_default_namespace")?,
            ),
            ("sql_lexical_syntax", syntax.clone()),
            (
                "sql_analysis_dialect",
                adapter.call_method0("sql_analysis_dialect")?,
            ),
        ],
    )
}
