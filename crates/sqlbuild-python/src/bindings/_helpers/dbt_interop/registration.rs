//! Register the static `sqb dbt` interop helpers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::dbt_interop::cli;
use crate::bindings::_helpers::dbt_interop::execution;
use crate::bindings::_helpers::dbt_interop::graph;
use crate::bindings::_helpers::dbt_interop::manifest;
use crate::bindings::_helpers::dbt_interop::orchestration;
use crate::bindings::_helpers::dbt_interop::plan;
use crate::bindings::_helpers::dbt_interop::profile;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cli::register(module)?;
    execution::register(module)?;
    graph::register(module)?;
    manifest::register(module)?;
    orchestration::register(module)?;
    plan::register(module)?;
    profile::register(module)?;
    super::profile_coercion::register(module)?;
    super::profile_connection::register(module)?;
    super::reference_resolver::register(module)?;
    super::runtime_config::register(module)?;
    super::runtime_execution::register(module)?;
    super::pipeline_prologue::register(module)?;
    super::plan_output::register(module)?;
    super::sqlbuild_work::register(module)?;
    super::pipeline_phases::register(module)?;
    super::runner::register(module)?;
    super::event_stream::register(module)?;
    super::pipeline::register(module)
}
