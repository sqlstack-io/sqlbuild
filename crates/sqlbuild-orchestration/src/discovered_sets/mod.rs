//! What `@discover` functions returned, kept in the snapshot so later compiles reuse it.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;
pub mod types;

#[cfg(test)]
#[path = "../../tests/discovered_sets/test_contracts.rs"]
mod tests;
