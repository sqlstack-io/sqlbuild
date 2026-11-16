//! Add migration fingerprints, rename hints, migration, table-type and retention entries.

use crate::planner_phases::_helpers::outputs::output_assembly;
use crate::planner_phases::models::{PlanOutputFields, StoragePolicyContext};
use pyo3::prelude::PyResult;

/// Add migration fingerprints, rename hints, migration, table-type and retention entries.
#[inline]
pub fn add_storage_policies<'py>(
    output: PlanOutputFields<'py>,
    StoragePolicyContext {
        runtime,
        warehouse,
        scopes,
        policies,
    }: StoragePolicyContext<'_, 'py>,
) -> PyResult<PlanOutputFields<'py>> {
    output_assembly::add_storage_policies(
        output,
        StoragePolicyContext {
            runtime,
            warehouse,
            scopes,
            policies,
        },
    )
}
