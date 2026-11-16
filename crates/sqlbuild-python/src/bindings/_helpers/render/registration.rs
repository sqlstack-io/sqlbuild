//! Register the macro call and macro argument functions and classes.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::render::main::calls::register_macro_arguments::register_macro_arguments;
use sqlbuild_python_front::render::main::calls::register_macro_calls::register_macro_calls;
use sqlbuild_python_front::render::main::declarations::register_authored_sql::register_authored_sql;
use sqlbuild_python_front::render::main::declarations::register_declaration_indexes::register_declaration_indexes;
use sqlbuild_python_front::render::main::declarations::register_declaration_values::register_declaration_values;
use sqlbuild_python_front::render::main::expansion::register_macro_expansion::register_macro_expansion;
use sqlbuild_python_front::render::main::expansion::register_macro_expansion_result::register_macro_expansion_result;
use sqlbuild_python_front::render::main::expansion::register_macro_modules::register_macro_modules;
use sqlbuild_python_front::render::main::sql_macros::register_sql_macro_inventory::register_sql_macro_inventory;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_authored_sql(module)?;
    register_declaration_values(module)?;
    register_declaration_indexes(module)?;
    register_macro_arguments(module)?;
    register_macro_calls(module)?;
    register_macro_expansion(module)?;
    register_macro_expansion_result(module)?;
    register_macro_modules(module)?;
    register_sql_macro_inventory(module)?;
    Ok(())
}
