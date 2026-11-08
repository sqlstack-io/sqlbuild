//! Which engine analyses a dialect's SQL: the Polyglot path, or a native module over SQLQuack.

pub(crate) mod _helpers;
pub(crate) mod constants;
pub mod main;
pub mod models;
pub mod types;

pub use crate::analysis_route::main::analysis_route::analysis_route;
pub use crate::analysis_route::models::{
    AnalysisInput, AnalysisRoute, NativeAnalysis, ParserGap, StandInSchema, TypeMatch,
};
pub use crate::analysis_route::types::NativeDialect;

#[cfg(test)]
mod tests;
