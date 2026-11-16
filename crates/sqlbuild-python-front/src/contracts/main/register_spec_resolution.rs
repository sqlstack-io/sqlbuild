//! Register the spec resolution functions and classes.

use crate::contracts::_helpers::spec_resolution::scenario_local_type_overrides_for_dialect;
use crate::contracts::main::loader_destination_parts::loader_destination_parts;
use crate::contracts::main::matching_dynamic_families::matching_dynamic_families;
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use crate::contracts::main::resolve_effective_collection_rendering::resolve_effective_collection_rendering;
use crate::contracts::main::resolve_effective_scenario_config::resolve_effective_scenario_config;
use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::contracts::main::resolve_target_name::resolve_target_name;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the spec resolution functions and classes.
pub fn register_spec_resolution(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_target_name, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_target_config, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_adapter_name, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_effective_collection_rendering,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_scenario_config, module)?)?;
    module.add_function(wrap_pyfunction!(
        scenario_local_type_overrides_for_dialect,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(loader_destination_parts, module)?)?;
    module.add_function(wrap_pyfunction!(matching_dynamic_families, module)?)?;
    Ok(())
}
