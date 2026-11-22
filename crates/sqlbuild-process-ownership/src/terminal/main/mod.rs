//! The safe API: hand a child its terminal before it runs.

#[cfg(unix)]
pub mod controlling_terminal;
