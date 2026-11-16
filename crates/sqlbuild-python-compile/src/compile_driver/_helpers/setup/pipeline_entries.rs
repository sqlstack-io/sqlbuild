//! Small pipeline entry points: analysis selection, project graphs and pipeline results.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Discovered Python node functions whose selectors need the Python graph.
const PYTHON_NODE_FUNCTIONS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];
/// Plan entry lists that are runtime work.
pub(crate) const PLAN_WORK_ENTRIES: [&str; 6] = [
    "model_entries",
    "seed_entries",
    "source_load_entries",
    "function_entries",
    "audit_entries",
    "test_entries",
];

/// Limit analysis when selectors cannot resolve through Python graph nodes.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_compile_analysis_selection<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_compile_analysis_selection",
        options,
        (&["options", "discovered_inputs"], &[]),
    )?;
    compile_analysis_selection(
        &arguments.value("options")?,
        &arguments.value("discovered_inputs")?,
    )
}

/// `resolve_compile_analysis_selection` with its arguments.
pub(crate) fn compile_analysis_selection<'py>(
    options: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = options.py();
    let mut has_python_nodes: bool = false;
    for name in PYTHON_NODE_FUNCTIONS {
        if native_field(discovered, name)?.is_truthy()? {
            has_python_nodes = true;
            break;
        }
    }
    let no_cache: Bound<'py, PyAny> = native_field(options, "no_cache")?;
    if (native_field(options, "resolve_python_run_selectors")?.is_truthy()? && has_python_nodes)
        || native_field(options, "sql_test_selection")?
            .getattr("names")?
            .is_truthy()?
    {
        return new_record_with(
            py,
            RecordId::CompileAnalysisSelection,
            &[("no_cache", no_cache)],
        );
    }
    new_record_with(
        py,
        RecordId::CompileAnalysisSelection,
        &[
            ("select", native_field(options, "select")?),
            ("exclude", native_field(options, "exclude")?),
            (
                "auto_load_sources",
                native_field(options, "auto_load_sources")?,
            ),
            ("no_cache", no_cache),
        ],
    )
}
