//! Which native module analyses a dialect's SQL over SQLQuack, if any does.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;
pub mod types;

#[cfg(test)]
mod tests;
