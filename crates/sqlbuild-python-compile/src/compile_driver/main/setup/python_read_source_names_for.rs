//! Return sources declared by the selected Python nodes and their Python upstreams.

use crate::compile_driver::_helpers::setup::python_plan_entries::node_graph;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Return sources declared by the selected Python nodes and their Python upstreams.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn python_read_source_names_for<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "python_read_source_names_for",
        options,
        (&["discovered_inputs", "python_node_names"], &[]),
    )?;
    let frozenset: Bound<'py, PyAny> = imported(py, "builtins", "frozenset")?;
    let selected: Bound<'py, PyAny> = arguments.value("python_node_names")?;
    if !selected.is_truthy()? {
        return frozenset.call0();
    }
    let graph: Bound<'py, PyAny> = node_graph(&arguments.value("discovered_inputs")?)?;
    let source: Bound<'py, PyAny> =
        host_entry(py, UserCodeHost::SqlResourceRefKind)?.getattr("SOURCE")?;
    let mut names: Vec<Bound<'py, PyAny>> = Vec::new();
    for reference in call_with(
        &native_field(&graph, "selected_sql_refs")?,
        &[("selected_names", selected)],
    )?
    .try_iter()?
    {
        let reference: Bound<'py, PyAny> = reference?;
        if native_field(&reference, "kind")?.eq(&source)? {
            names.push(native_field(&reference, "name")?);
        }
    }
    frozenset.call1((PyTuple::new(py, names)?,))
}
