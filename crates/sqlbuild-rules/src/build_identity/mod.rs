//! The running native build's content identity, installed by the binary at startup instead of compiled in.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod errors;
pub mod main;

#[cfg(test)]
mod tests;
