//! Register the native dbt event stream.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::dbt_events::main::render_dbt_node_result::render_dbt_node_result;
use crate::dbt_events::models::{DbtEventStream, DbtEventStreamUpdate};

/// Register the dbt event stream classes and the node row renderer.
pub fn register_dbt_events(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DbtEventStream>()?;
    module.add_class::<DbtEventStreamUpdate>()?;
    module.add_function(wrap_pyfunction!(render_dbt_node_result, module)?)?;
    Ok(())
}
