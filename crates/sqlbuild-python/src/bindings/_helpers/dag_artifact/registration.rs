//! Register the DAG artifact builder.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::dagster_invocation;
use sqlbuild_python_compile::dag_artifact::main::register_artifact::register_artifact;
use sqlbuild_python_compile::dag_artifact::main::register_projection::register_projection;
use sqlbuild_python_compile::dag_artifact::main::register_selection::register_selection;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_artifact(module)?;
    register_projection(module)?;
    register_selection(module)?;
    dagster_invocation::registration::register(module)
}
