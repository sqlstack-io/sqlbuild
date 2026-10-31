//! Hashed, revocable credentials scoped to one principal, location, or claimed run.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
#[path = "../../tests/credentials/test_contracts.rs"]
pub(crate) mod tests;
