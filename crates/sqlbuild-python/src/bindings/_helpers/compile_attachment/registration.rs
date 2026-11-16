//! Register the compile-input attachment functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

use sqlbuild_python_compile::compile_attachment::main::build_compile_target_context_function::build_compile_target_context_function;
use sqlbuild_python_compile::compile_attachment::main::build_declaration_scope_function::build_declaration_scope_function;
use sqlbuild_python_compile::compile_attachment::main::build_model_inputs_function::build_model_inputs_function;
use sqlbuild_python_compile::compile_attachment::main::build_sql_expansion_context_function::build_sql_expansion_context_function;
use sqlbuild_python_compile::compile_attachment::main::native_model_config_session_function::native_model_config_session_function;
use sqlbuild_python_compile::compile_attachment::main::rebind_declaration_scope_function::rebind_declaration_scope_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_config_function::build_effective_connection_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_connection_function::build_effective_connection_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_runtime_config_function::build_effective_runtime_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings_config_function::build_effective_settings_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_settings_function::build_effective_settings_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_target_config_function::build_effective_target_config_function;
use sqlbuild_python_front::compile_attachment::main::build_effective_vars_function::build_effective_vars_function;
use sqlbuild_python_project::compile_attachment::main::resolve_cursor_input_roles_function::resolve_cursor_input_roles_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(build_sql_expansion_context_function(module)?)?;
    module.add_function(build_effective_connection_function(module)?)?;
    module.add_function(build_effective_settings_function(module)?)?;
    module.add_function(build_effective_vars_function(module)?)?;
    module.add_function(build_effective_target_config_function(module)?)?;
    module.add_function(build_effective_connection_config_function(module)?)?;
    module.add_function(build_effective_runtime_config_function(module)?)?;
    module.add_function(build_effective_settings_config_function(module)?)?;
    module.add_function(native_model_config_session_function(module)?)?;
    module.add_function(build_model_inputs_function(module)?)?;
    module.add_function(build_compile_target_context_function(module)?)?;
    module.add_function(build_declaration_scope_function(module)?)?;
    module.add_function(rebind_declaration_scope_function(module)?)?;
    module.add_function(resolve_cursor_input_roles_function(module)?)?;
    Ok(())
}
