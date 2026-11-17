//! Register the native declaration scope functions and classes.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::scopes::scope_command;
use sqlbuild_python_compile::scopes::main::register_scope_loading::register_scope_loading;
use sqlbuild_python_front::scopes::main::checks::register_scope_validation::register_scope_validation;
use sqlbuild_python_front::scopes::main::checks::register_scope_visibility::register_scope_visibility;
use sqlbuild_python_front::scopes::main::declarations::register_declaration_contexts::register_declaration_contexts;
use sqlbuild_python_front::scopes::main::index::register_scope_cache::register_scope_cache;
use sqlbuild_python_front::scopes::main::index::register_scope_index::register_scope_index;
use sqlbuild_python_front::scopes::main::index::register_scope_lookup::register_scope_lookup;
use sqlbuild_python_front::scopes::main::relationships::register_compile_usages::register_compile_usages;
use sqlbuild_python_front::scopes::main::relationships::register_relationship_names::register_relationship_names;
use sqlbuild_python_front::scopes::main::reports::register_scope_reports::register_scope_reports;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_scope_index(module)?;
    register_relationship_names(module)?;
    register_declaration_contexts(module)?;
    register_compile_usages(module)?;
    register_scope_lookup(module)?;
    register_scope_cache(module)?;
    register_scope_loading(module)?;
    register_scope_reports(module)?;
    scope_command::register(module)?;
    register_scope_validation(module)?;
    register_scope_visibility(module)?;
    Ok(())
}
