//! Borrowed Python callback boundaries for scenario SQL assembly.

use sqlbuild_python_base::native_host::models::Dialect;
use pyo3::prelude::{Bound, PyAny};

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
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) graph_plan: Bound<'py, PyAny>,
    pub(crate) relation_map: Bound<'py, PyAny>,
    pub(crate) render_qualified_name: Bound<'py, PyAny>,
    pub(crate) database: Bound<'py, PyAny>,
    pub(crate) schema: Bound<'py, PyAny>,
}
