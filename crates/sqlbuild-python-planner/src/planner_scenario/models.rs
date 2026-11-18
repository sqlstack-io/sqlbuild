//! Public records of planner scenario.

use pyo3::prelude::{Bound, PyAny};
use sqlbuild_python_base::native_host::models::Dialect;

#[derive(Debug)]
pub(crate) struct SqlContext<'py> {
    pub dialect: Bound<'py, Dialect>,
    pub relations: Bound<'py, PyAny>,
    pub syntax: Bound<'py, PyAny>,
    pub label: String,
}

/// One scenario's graph and the relation map and qualification that locate its relations.
#[derive(Debug)]
pub(crate) struct ScenarioRelations<'py> {
    pub project: Bound<'py, PyAny>,
    pub graph_plan: Bound<'py, PyAny>,
    pub relation_map: Bound<'py, PyAny>,
    pub render_qualified_name: Bound<'py, PyAny>,
    pub database: Bound<'py, PyAny>,
    pub schema: Bound<'py, PyAny>,
}
