//! Which engine analyses a dialect's SQL: the Polyglot path, or a native module over SQLQuack.

mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;
pub mod types;

#[cfg(test)]
mod tests;
