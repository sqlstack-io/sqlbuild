//! Native project discovery: the directory walk, file facts and parsing of authored files.

#![forbid(unsafe_code)]

pub(crate) mod _helpers;
pub mod constants;
pub mod declaration_files;
pub mod declarations;
pub mod header_changes;
pub mod key_checks;
pub mod model_files;
pub mod models;
pub mod orchestration_files;
pub mod project_validation;
pub mod python_paths;
pub mod sql_tests;
pub mod tree;
pub mod yaml_entries;
pub mod yaml_files;
