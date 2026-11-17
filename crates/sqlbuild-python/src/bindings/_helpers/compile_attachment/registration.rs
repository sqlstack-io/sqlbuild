//! Register the compile-input attachment functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::bindings::_helpers::compile_attachment::cursor_roles;
use crate::bindings::_helpers::compile_attachment::declaration_scope;
use crate::bindings::_helpers::compile_attachment::model_config;
use crate::bindings::_helpers::compile_attachment::model_inputs;
use crate::bindings::_helpers::compile_attachment::sql_expansion_context;
use crate::bindings::_helpers::compile_attachment::target;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_config_function::build_effective_connection_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_function::build_effective_connection_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_runtime_config_function::build_effective_runtime_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings_config_function::build_effective_settings_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings_function::build_effective_settings_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_target_config_function::build_effective_target_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_vars_function::build_effective_vars_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        sql_expansion_context::build_sql_expansion_context,
        module
    )?)?;
    module.add_function(build_effective_connection_function(module)?)?;
    module.add_function(build_effective_settings_function(module)?)?;
    module.add_function(build_effective_vars_function(module)?)?;
    module.add_function(build_effective_target_config_function(module)?)?;
    module.add_function(build_effective_connection_config_function(module)?)?;
    module.add_function(build_effective_runtime_config_function(module)?)?;
    module.add_function(build_effective_settings_config_function(module)?)?;
    module.add_function(wrap_pyfunction!(
        model_config::native_model_config_session,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(model_inputs::build_model_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(
        target::build_compile_target_context,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        target::build_effective_target_namespace,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        declaration_scope::build_declaration_scope,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        declaration_scope::rebind_declaration_scope,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        cursor_roles::resolve_cursor_input_roles,
        module
    )?)?;
    Ok(())
}
