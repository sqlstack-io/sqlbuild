//! Register the projection functions and classes.

use crate::dag_artifact::_helpers::projection::dagster_asset_plans;
use crate::dag_artifact::_helpers::projection::dagster_check_plans;
use crate::dag_artifact::_helpers::projection::dagster_selected_asset_nodes;
use crate::dag_artifact::_helpers::projection::rivers_asset_plans;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the projection functions and classes.
pub fn register_projection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(dagster_asset_plans, module)?)?;
    module.add_function(wrap_pyfunction!(dagster_check_plans, module)?)?;
    module.add_function(wrap_pyfunction!(dagster_selected_asset_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(rivers_asset_plans, module)?)?;
    Ok(())
}
