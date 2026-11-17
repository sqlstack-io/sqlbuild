//! Register the DAG artifact builder.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::dag_artifact::artifact;
use crate::bindings::_helpers::dag_artifact::projection;
use crate::bindings::_helpers::dag_artifact::selection;
use crate::bindings::_helpers::dagster_invocation;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    artifact::register(module)?;
    projection::register(module)?;
    selection::register(module)?;
    dagster_invocation::registration::register(module)
}
