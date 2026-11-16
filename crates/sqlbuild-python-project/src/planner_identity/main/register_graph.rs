//! Register the graph functions and classes.

use crate::planner_identity::_helpers::graph::build_downstream_deps;
use crate::planner_identity::_helpers::graph::build_downstream_model_name_closure;
use crate::planner_identity::_helpers::graph::build_execution_edge_origins;
use crate::planner_identity::_helpers::graph::build_execution_upstream_deps;
use crate::planner_identity::_helpers::graph::check_buildability;
use crate::planner_identity::_helpers::graph::missing_upstream_message;
use crate::planner_identity::_helpers::graph::topologically_order_keys;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the graph functions and classes.
pub fn register_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_execution_upstream_deps, module)?)?;
    module.add_function(wrap_pyfunction!(build_execution_edge_origins, module)?)?;
    module.add_function(wrap_pyfunction!(build_downstream_deps, module)?)?;
    module.add_function(wrap_pyfunction!(topologically_order_keys, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_downstream_model_name_closure,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(check_buildability, module)?)?;
    module.add_function(wrap_pyfunction!(missing_upstream_message, module)?)
}
