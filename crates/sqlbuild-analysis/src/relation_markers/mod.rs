//! Relation marker calls, such as `__ref("orders")`, read as plain relation names by strict parsers.

mod _helpers;
pub mod constants;
pub mod main;
pub mod types;

#[cfg(test)]
mod tests;
