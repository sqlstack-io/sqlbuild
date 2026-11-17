//! Resolve canonical locations and source routing without warehouse inventory.

use crate::planner_entries::_helpers::entries::relations::planner_relations_context;
<<<<<<< HEAD
=======
use crate::planner_entries::_helpers::entries::values::none;
>>>>>>> origin/main
use crate::planner_entries::models::RelationsRequest;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;
<<<<<<< HEAD
use sqlbuild_python_base::native_host::models::Dialect;
=======
>>>>>>> origin/main

/// Resolve canonical locations and source routing without warehouse inventory.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub fn resolve_static_relation_context<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_static_relation_context",
        kwargs,
        (
<<<<<<< HEAD
            &["project", "dialect", "scope"],
=======
            &["project", "adapter", "scope"],
>>>>>>> origin/main
            &[
                "deferral",
                "project_config",
                "local_config",
                "relation_keys",
            ],
        ),
    )?;
    planner_relations_context(RelationsRequest {
        project: args.value("project")?,
<<<<<<< HEAD
        dialect: Dialect::passed(args.value("dialect")?)?,
        reads: None,
=======
        adapter: args.value("adapter")?,
        connection: none(py),
>>>>>>> origin/main
        scope: args.value("scope")?,
        deferral: args.value("deferral")?,
        project_config: args.value("project_config")?,
        local_config: args.value("local_config")?,
        known_source_columns: PyDict::new(py).into_any(),
        relation_keys: args.value("relation_keys")?,
    })
}
