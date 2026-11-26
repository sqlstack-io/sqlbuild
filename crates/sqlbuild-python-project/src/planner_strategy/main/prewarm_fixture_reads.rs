//! Resolve the fixture reads of a batch of tests' models ahead of per-test planning.

use crate::planner_strategy::_helpers::fixtures::fixture_reads;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use pyo3::prelude::{Bound, PyResult};
use sqlbuild_python_base::native_host::models::Dialect;

/// Resolve, in parallel, the reads of every listed model that per-test planning can look up.
pub fn prewarm_fixture_reads(
    context: &RelationFixturePlanningContext,
    model_names: &[String],
    dialect: &Bound<'_, Dialect>,
) -> PyResult<()> {
    fixture_reads::prewarm_fixture_reads(context, model_names, dialect)
}
