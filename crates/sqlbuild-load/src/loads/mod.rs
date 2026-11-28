//! Loads: a sources file's `load:` read as a SQLLoad source, checked by SQLLoad when the project compiles.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
mod tests;
