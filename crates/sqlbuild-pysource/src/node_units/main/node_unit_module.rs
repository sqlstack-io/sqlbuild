//! Read a module under `python/` for the node unit rules, without importing it.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::node_units::_helpers::functions::{last_name, leading_docstring, unit_function};
use crate::node_units::_helpers::imports::{is_type_checking_imports, module_imports};
use crate::node_units::_helpers::positions::{UnitText, unit_position};
use crate::node_units::models::{UnitClass, UnitModule, UnitStatement, UnitStatementKind};

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
                Stmt::If(block) if is_type_checking_imports(block) => UnitStatementKind::Import,
                Stmt::FunctionDef(function) => {
                    UnitStatementKind::Function(unit_function(function, &source))
                }
                Stmt::ClassDef(class) => UnitStatementKind::Class(UnitClass {
                    name: class.name.to_string(),
                    position: unit_position(&source, class.name.range().start()),
                    bases: class.bases().iter().filter_map(last_name).collect(),
                }),
                Stmt::Assign(assign) => UnitStatementKind::Assignment(
                    assign.targets.iter().flat_map(target_names).collect(),
                ),
                Stmt::AnnAssign(assign) => {
                    UnitStatementKind::Assignment(target_names(&assign.target))
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

/// The names an assignment target binds: a plain name, or the names inside a tuple, list or starred target.
fn target_names(target: &Expr) -> Vec<String> {
    match target {
        Expr::Name(name) => vec![name.id.to_string()],
        Expr::Tuple(tuple) => tuple.elts.iter().flat_map(target_names).collect(),
        Expr::List(list) => list.elts.iter().flat_map(target_names).collect(),
        Expr::Starred(starred) => target_names(&starred.value),
        _ => Vec::new(),
    }
}
