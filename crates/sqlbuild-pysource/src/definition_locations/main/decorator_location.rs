//! Where a named decorator of a function sits in a module source.

use ruff_python_ast::Stmt;
use ruff_text_size::Ranged;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::main::source_position::source_position;
use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::{CpythonNode, ParsedSource};
use crate::definition_locations::_helpers::decorators::decorator_name;
use crate::definition_locations::constants::MISSING_DECORATOR_LOCATION;

/// The `lineno` and byte `col_offset` of the first `@decorator` in `ast.walk` order, else `(1, 1)`.
#[must_use]
pub fn decorator_location(
    contents: &str,
    decorator: &str,
    python_version: (u8, u8),
) -> (usize, usize) {
    let Some(parsed): Option<ParsedSource> = parse_cpython_module(contents, python_version) else {
        return MISSING_DECORATOR_LOCATION;
    };
    for node in walk_breadth_first(parsed.module.body.iter().map(CpythonNode::Stmt).collect()) {
        let CpythonNode::Stmt(Stmt::FunctionDef(function)) = node else {
            continue;
        };
        for expression in function.decorator_list.iter().map(|item| &item.expression) {
            if decorator_name(expression) == Some(decorator) {
                return source_position(&parsed.lines, expression.range().start());
            }
        }
    }
    MISSING_DECORATOR_LOCATION
}
