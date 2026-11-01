//! Sensor attempts and cursor commits are immutable facts, not mutable lifecycle rows.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
#[path = "../../tests/sensors/test_contracts.rs"]
mod tests;
