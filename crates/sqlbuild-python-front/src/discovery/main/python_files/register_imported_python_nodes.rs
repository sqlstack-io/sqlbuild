//! Register the imported python nodes functions and classes.

use crate::discovery::_helpers::python_files::imported_providers::bind_event_exporter_declarations;
use crate::discovery::_helpers::python_files::imported_providers::discover_event_exporter_declarations;
use crate::discovery::_helpers::python_files::imported_providers::discover_provider_classes;
use crate::discovery::_helpers::python_files::imported_python_nodes::discover_hook_functions;
use crate::discovery::main::python_files::discover_event_exporter_functions::discover_event_exporter_functions;
use crate::discovery::main::python_files::discover_materialization_files::discover_materialization_files;
use crate::discovery::main::python_files::discover_python_node_functions::discover_python_node_functions;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the imported python nodes functions and classes.
pub fn register_imported_python_nodes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_python_node_functions, module)?)?;
    module.add_function(wrap_pyfunction!(discover_hook_functions, module)?)?;
    module.add_function(wrap_pyfunction!(discover_materialization_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_provider_classes, module)?)?;
    module.add_function(wrap_pyfunction!(
        discover_event_exporter_declarations,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(bind_event_exporter_declarations, module)?)?;
    module.add_function(wrap_pyfunction!(discover_event_exporter_functions, module)?)
}
