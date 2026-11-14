//! Static reading of project Python source; CPython stays the authority on syntax validity.

#![forbid(unsafe_code)]

pub(crate) mod cpython_tree;
pub mod definition_locations;
pub mod import_rewrites;
pub mod literal_sql;
pub mod macro_modules;
pub mod module_parse;
pub mod orchestration_modules;
pub mod rule_evidence;
pub mod string_literals;
pub mod udf_functions;
