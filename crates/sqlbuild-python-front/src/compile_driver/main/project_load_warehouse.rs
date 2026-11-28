//! The SQLLoad warehouse a project's loads and loaders write to.

use pyo3::prelude::{PyAnyMethods, PyResult};
use sqlbuild_load::warehouses::main::load_warehouse::load_warehouse;
use sqlload::column_types::models::Warehouse;

use crate::compile_driver::models::ProjectConfiguration;
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;

/// The project's warehouse, or the error naming an adapter SQLLoad can't load into.
pub fn project_load_warehouse(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<Result<Warehouse, String>> {
    let adapter: String =
        resolve_effective_adapter_name(config.project_config, config.local_config)?
            .str()?
            .to_string();
    Ok(load_warehouse(&adapter).map(|(warehouse, _)| warehouse))
}
