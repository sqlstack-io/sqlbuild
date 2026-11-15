//! Module-level imports and whether each bound name is used only to name a dependency.

use ruff_python_ast::{Expr, Stmt, StmtIf};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::cpython_tree::main::walk_breadth_first::walk_breadth_first;
use crate::cpython_tree::models::CpythonNode;
use crate::node_units::_helpers::positions::{UnitText, unit_position};
use crate::node_units::constants::{DEPENDS_ON_KEYWORD, TYPE_CHECKING_NAME};
use crate::node_units::models::UnitImport;

/// Every name the module's top-level imports bind, in source order.
pub(crate) fn module_imports(body: &[Stmt], source: &UnitText<'_>) -> Vec<UnitImport> {
    let nodes: Vec<CpythonNode<'_>> =
        walk_breadth_first(body.iter().map(CpythonNode::Stmt).collect());
    let dependency_ranges: Vec<TextRange> = depends_on_ranges(&nodes);
    let mut imports: Vec<UnitImport> = Vec::new();
    for statement in body.iter().flat_map(import_statements) {
        match statement {
            Stmt::Import(import) => imports.extend(import.names.iter().map(|alias| UnitImport {
                module: alias.name.to_string(),
                level: 0,
                name: None,
                local: alias.asname.as_ref().map_or_else(
                    || {
                        alias
                            .name
                            .as_str()
                            .split('.')
                            .next()
                            .unwrap_or_default()
                            .to_owned()
                    },
                    ToString::to_string,
                ),
                position: unit_position(source, alias.range().start()),
                only_in_depends_on: false,
            })),
            Stmt::ImportFrom(import) => imports.extend(import.names.iter().map(|alias| {
                UnitImport {
                    module: import
                        .module
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    level: import.level,
                    name: Some(alias.name.to_string()),
                    local: alias.asname.as_ref().unwrap_or(&alias.name).to_string(),
                    position: unit_position(source, alias.range().start()),
                    only_in_depends_on: false,
                }
            })),
            _ => {}
        }
    }
    for import in &mut imports {
        import.only_in_depends_on = used_only_in(&nodes, &import.local, &dependency_ranges);
    }
    imports
}

/// Whether `local` is loaded at least once, and every load sits inside one of `ranges`.
fn used_only_in(nodes: &[CpythonNode<'_>], local: &str, ranges: &[TextRange]) -> bool {
    let mut uses = name_uses(nodes, local).peekable();
    uses.peek().is_some() && uses.all(|start| within_any(ranges, start))
}

/// `statement` itself, or the imports of an import-only `if TYPE_CHECKING:` block.
fn import_statements(statement: &Stmt) -> &[Stmt] {
    match statement {
        Stmt::If(block) if is_type_checking_imports(block) => &block.body,
        other => std::slice::from_ref(other),
    }
}

/// Whether `block` is `if TYPE_CHECKING:` or `if typing.TYPE_CHECKING:`, holding only imports and no other branch.
pub(crate) fn is_type_checking_imports(block: &StmtIf) -> bool {
    let guard: bool = match block.test.as_ref() {
        Expr::Name(name) => name.id.as_str() == TYPE_CHECKING_NAME,
        Expr::Attribute(attribute) => attribute.attr.as_str() == TYPE_CHECKING_NAME,
        _ => false,
    };
    guard
        && block.elif_else_clauses.is_empty()
        && block
            .body
            .iter()
            .all(|statement| matches!(statement, Stmt::Import(_) | Stmt::ImportFrom(_)))
}

fn within_any(ranges: &[TextRange], offset: TextSize) -> bool {
    ranges.iter().any(|range| range.contains(offset))
}

/// The start of every load of `local` in the module.
fn name_uses<'a>(
    nodes: &'a [CpythonNode<'a>],
    local: &'a str,
) -> impl Iterator<Item = TextSize> + 'a {
    nodes.iter().filter_map(move |node| match node {
        CpythonNode::Expr(Expr::Name(name)) if name.id.as_str() == local => {
            Some(name.range().start())
        }
        _ => None,
    })
}

/// The value range of every `depends_on=` keyword in the module.
fn depends_on_ranges(nodes: &[CpythonNode<'_>]) -> Vec<TextRange> {
    nodes
        .iter()
        .filter_map(|node| match node {
            CpythonNode::Keyword(keyword)
                if keyword
                    .arg
                    .as_ref()
                    .is_some_and(|arg| arg.as_str() == DEPENDS_ON_KEYWORD) =>
            {
                Some(keyword.value.range())
            }
            _ => None,
        })
        .collect()
}
