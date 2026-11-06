//! Python-free commands `sqb` runs itself: help, usage errors and the commands needing no Python.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod errors;
pub mod main;
pub(crate) mod models;
pub(crate) mod types;

#[cfg(test)]
mod tests;
