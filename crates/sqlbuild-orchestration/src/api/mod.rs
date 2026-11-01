//! Python-free HTTP endpoints; locations and workers have no direct database access.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod errors;
pub mod main;
pub mod models;

#[cfg(test)]
#[path = "../../tests/api/test_contracts.rs"]
pub(crate) mod tests;
