//! Source connections: `[connections.*]` entries with a `kind` and no warehouse use, read into SQLLoad's connection types.

pub(crate) mod _helpers;
pub mod main;

#[cfg(test)]
mod tests;
