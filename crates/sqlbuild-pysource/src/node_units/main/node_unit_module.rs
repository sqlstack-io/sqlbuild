//! Read a module under `python/` for the node unit rules, without importing it.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::node_units::_helpers::functions::{leading_docstring, unit_function};
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
                    bases: class
                        .bases()
                        .iter()
                        .map(|base| dotted_name(base).unwrap_or_default())
                        .collect(),
                }),
                Stmt::Assign(assign) => UnitStatementKind::Assignment(
                    assign
                        .targets
                        .iter()
                        .map(target_names)
                        .collect::<Option<Vec<Vec<String>>>>()
                        .map(|names| names.concat())
                        .unwrap_or_default(),
                ),
                Stmt::AnnAssign(assign) => {
                    UnitStatementKind::Assignment(target_names(&assign.target).unwrap_or_default())
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

/// The names an assignment target binds, through tuples, lists and starred targets; `None` when any part
/// of it is an attribute or subscript.
fn target_names(target: &Expr) -> Option<Vec<String>> {
    match target {
        Expr::Name(name) => Some(vec![name.id.to_string()]),
        Expr::Tuple(tuple) => elements_names(&tuple.elts),
        Expr::List(list) => elements_names(&list.elts),
        Expr::Starred(starred) => target_names(&starred.value),
        _ => None,
    }
}

/// The names the elements of a tuple or list target bind; `None` when any of them binds no plain name.
fn elements_names(elements: &[Expr]) -> Option<Vec<String>> {
    elements
        .iter()
        .map(target_names)
        .collect::<Option<Vec<Vec<String>>>>()
        .map(|names| names.concat())
}

/// A name or attribute chain as dotted text, such as `paged.PagedApi`, seen through a subscript such as
/// `Generic[T]`; `None` for any other expression.
fn dotted_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => Some(format!(
            "{}.{}",
            dotted_name(&attribute.value)?,
            attribute.attr
        )),
        Expr::Subscript(subscript) => dotted_name(&subscript.value),
        _ => None,
    }
}
