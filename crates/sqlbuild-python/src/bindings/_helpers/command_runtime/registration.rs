//! Register command connection and warehouse resolution.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::command_runtime::{
    connection, preview_target, warehouse_scope, warehouses,
};
use sqlbuild_python_base::command_runtime::main::register_adapters::register_adapters;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_adapters(module)?;
    warehouse_scope::register(module)?;
    warehouses::register(module)?;
    connection::register(module)?;
    preview_target::register(module)
}
