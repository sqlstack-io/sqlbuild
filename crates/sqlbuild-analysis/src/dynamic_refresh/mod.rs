//! Dynamic refresh: dynamic table lag text, refresh setting drift, and readers a rebuild reinitialises.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;

#[cfg(test)]
mod tests;
