//! Where unit tests and scenarios run: locally in DuckDB, or on the warehouse, and why.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
mod tests;
