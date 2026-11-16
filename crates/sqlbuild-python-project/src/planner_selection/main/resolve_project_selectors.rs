//! Model, source, seed and function selectors resolved against compiled graph mappings.

use crate::graph::models::NativeProjectGraph;
use crate::planner_selection::_helpers::selectors::VIEW_OPTIONS;
use crate::planner_selection::_helpers::selectors::arguments_graph;
use crate::planner_selection::_helpers::selectors::resolved;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Model, source, seed and function selectors resolved against compiled graph mappings.
pub fn resolve_project_selectors<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_project_selectors",
        options,
        (&[&["select", "exclude"], VIEW_OPTIONS].concat(), &[]),
    )?;
    let graph: NativeProjectGraph = arguments_graph(&arguments)?;
    resolved(
        graph.graph(),
        (&arguments.value("select")?, &arguments.value("exclude")?),
    )
}
