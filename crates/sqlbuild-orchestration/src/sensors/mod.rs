//! Sensor attempts are append-only facts; the latest cursor lives on the trigger's state.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;
pub(crate) mod types;

#[cfg(test)]
#[path = "../../tests/sensors/test_contracts.rs"]
pub(crate) mod tests;
