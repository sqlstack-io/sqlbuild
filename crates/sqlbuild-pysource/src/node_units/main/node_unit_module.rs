//! Read a module under `python/` for the node unit rules, without importing it.

use ruff_python_ast::Stmt;
use ruff_text_size::Ranged;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::node_units::_helpers::functions::{leading_docstring, unit_function};
use crate::node_units::_helpers::imports::module_imports;
use crate::node_units::_helpers::positions::{UnitText, unit_position};
use crate::node_units::models::{UnitModule, UnitStatement, UnitStatementKind};

/// The top-level statements and imports of `contents`, or `None` when it does not parse.
#[must_use]
pub fn node_unit_module(contents: &str, python_version: (u8, u8)) -> Option<UnitModule> {
    let parsed: ParsedSource = parse_cpython_module(contents, python_version)?;
    let text: String = contents.replace("\r\n", "\n").replace('\r', "\n");
    let source = UnitText {
        text: &text,
        lines: &parsed.lines,
    };
    let body: &[Stmt] = &parsed.module.body;
    let has_docstring: bool = leading_docstring(body);
    let statements: Vec<UnitStatement> = body
        .iter()
        .enumerate()
        .map(|(index, statement)| UnitStatement {
            kind: match statement {
                _ if index == 0 && has_docstring => UnitStatementKind::Docstring,
                Stmt::Import(_) | Stmt::ImportFrom(_) => UnitStatementKind::Import,
                Stmt::FunctionDef(function) => {
                    UnitStatementKind::Function(unit_function(function, &source))
                }
                _ => UnitStatementKind::Other,
            },
            position: unit_position(&source, statement.range().start()),
        })
        .collect();
    Some(UnitModule {
        statements,
        imports: module_imports(body, &source),
    })
}
