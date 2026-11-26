//! Each server's own health, kept in memory so it answers while the control database is unreachable, with the real cause of each failure.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;
#[cfg(test)]
#[path = "../../tests/health/test_contracts.rs"]
mod tests;
