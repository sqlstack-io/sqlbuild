//! A run's bulk output in zstd objects in an output store, with a small index in the control store.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;
pub mod types;

#[cfg(test)]
#[path = "../../tests/run_output/test_contracts.rs"]
pub(crate) mod tests;
