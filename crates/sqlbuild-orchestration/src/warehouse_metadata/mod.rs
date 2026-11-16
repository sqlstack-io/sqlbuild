//! Warehouse-owned observations mirrored by authenticated code locations, never execution state.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
#[path = "../../tests/warehouse_metadata/test_contracts.rs"]
mod tests;
