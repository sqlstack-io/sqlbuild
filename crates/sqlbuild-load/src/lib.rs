//! Loads, loaders and sinks on SQLLoad's engine: SQLBuild's source connections and load blocks read into SQLLoad's types, and SQLBuild's warehouse connections as SQLLoad's caller sessions. Only the Python extension links it.

#![forbid(unsafe_code)]

pub mod literal_settings;
pub mod loads;
pub mod source_connections;
pub mod warehouses;
