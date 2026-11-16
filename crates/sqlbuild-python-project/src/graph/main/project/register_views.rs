//! Register the views functions and classes.

use crate::graph::_helpers::project::views::lineage_graph_names;
use crate::graph::_helpers::project::views::lineage_graph_views;
use crate::graph::_helpers::project::views::native_graph_from_views;
use crate::graph::_helpers::project::views::native_graph_keys;
use crate::graph::main::project::compiled_graph_keys::compiled_graph_keys;
use crate::graph::main::project::native_graph_from_edges::native_graph_from_edges;
use crate::graph::main::project::project_lineage_views::project_lineage_views;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the views functions and classes.
pub fn register_views(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(native_graph_keys, module)?)?;
    module.add_function(wrap_pyfunction!(compiled_graph_keys, module)?)?;
    module.add_function(wrap_pyfunction!(lineage_graph_names, module)?)?;
    module.add_function(wrap_pyfunction!(lineage_graph_views, module)?)?;
    module.add_function(wrap_pyfunction!(project_lineage_views, module)?)?;
    module.add_function(wrap_pyfunction!(native_graph_from_views, module)?)?;
    module.add_function(wrap_pyfunction!(native_graph_from_edges, module)?)?;
    Ok(())
}
