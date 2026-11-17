//! Register the algorithms functions and classes.

use crate::graph::_helpers::nodes::algorithms::invert_edges;
use crate::graph::_helpers::nodes::algorithms::path_nodes;
use crate::graph::_helpers::nodes::algorithms::transitive_closure;
use crate::graph::main::nodes::transitive_closure_many::transitive_closure_many;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the algorithms functions and classes.
pub fn register_algorithms(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(transitive_closure, module)?)?;
    module.add_function(wrap_pyfunction!(transitive_closure_many, module)?)?;
    module.add_function(wrap_pyfunction!(invert_edges, module)?)?;
    module.add_function(wrap_pyfunction!(path_nodes, module)?)?;
    Ok(())
}
