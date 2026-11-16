//! Per-compile state below the compile driver: collectors, input reads and the delta-compile slots.

pub(crate) mod _helpers;
pub mod constants;
pub mod main;
pub mod models;
pub mod types;

#[cfg(test)]
mod tests;
