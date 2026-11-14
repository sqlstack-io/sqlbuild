//! The top-level functions of a module that carry one of the named decorators.

use ruff_python_ast::{Stmt, StmtFunctionDef};

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::definition_locations::_helpers::decorators::decorator_name;

/// Names of top-level functions with a decorator whose last name segment is in `decorators`; none when the module does not parse.
#[must_use]
pub fn decorated_functions(
    contents: &str,
    decorators: &[&str],
    python_version: (u8, u8),
) -> Vec<String> {
    let Some(parsed): Option<ParsedSource> = parse_cpython_module(contents, python_version) else {
        return Vec::new();
    };
    parsed
        .module
        .body
        .iter()
        .filter_map(|statement| match statement {
            Stmt::FunctionDef(function) => Some(function),
            _ => None,
        })
        .filter(|function| has_decorator(function, decorators))
        .map(|function| function.name.id.to_string())
        .collect()
}

fn has_decorator(function: &StmtFunctionDef, decorators: &[&str]) -> bool {
    function
        .decorator_list
        .iter()
        .filter_map(|item| decorator_name(&item.expression))
        .any(|name| decorators.contains(&name))
}
