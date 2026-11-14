//! A sensor condition's selection, following names bound at module level and noting names imported from elsewhere.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, ExprCall, Stmt};
use ruff_text_size::{Ranged, TextSize};

use crate::orchestration_modules::_helpers::calls::{authoring_call, literal, position};
use crate::orchestration_modules::constants::{
    EXCERPT_LIMIT, SELECT_KEYWORD, SELECTING_CONDITION_CALLS,
};
use crate::orchestration_modules::models::{ConditionSource, LiteralArgument, ModuleScope};

/// Each module-level `NAME = value` or `NAME: T = value` with one plain name target, in source order.
pub(crate) fn module_assignments(body: &[Stmt]) -> BTreeMap<&str, Vec<(TextSize, &Expr)>> {
    let mut assignments: BTreeMap<&str, Vec<(TextSize, &Expr)>> = BTreeMap::new();
    for statement in body {
        if let Some((name, value)) = assignment(statement) {
            assignments
                .entry(name)
                .or_default()
                .push((statement.range().start(), value));
        }
    }
    assignments
}

/// Each name a module-level `from ... import` binds, with its module, relative level and imported name.
pub(crate) fn module_imports(body: &[Stmt]) -> BTreeMap<&str, (String, u32, &str)> {
    let mut imports: BTreeMap<&str, (String, u32, &str)> = BTreeMap::new();
    for statement in body {
        let Stmt::ImportFrom(import) = statement else {
            continue;
        };
        let module: String = import
            .module
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        for alias in &import.names {
            let local: &str = alias.asname.as_ref().unwrap_or(&alias.name).as_str();
            imports.insert(local, (module.clone(), import.level, alias.name.as_str()));
        }
    }
    imports
}

/// What `value` selects when it runs at `before`: a selecting call's `select`, a name imported from elsewhere, or what cannot be followed.
pub(crate) fn condition_source(
    value: &Expr,
    before: TextSize,
    scope: &ModuleScope<'_>,
) -> Option<ConditionSource> {
    let mut current: (&Expr, TextSize) = (value, before);
    loop {
        let (expression, at) = current;
        match expression {
            Expr::NoneLiteral(_) => return None,
            Expr::Call(call) => return call_source(call, scope),
            Expr::Name(name) => {
                let id: &str = name.id.as_str();
                let bound = scope.assignments.get(id).and_then(|bindings| {
                    bindings
                        .iter()
                        .rev()
                        .find(|(start, _)| *start < at)
                        .copied()
                });
                if let Some((start, bound)) = bound {
                    current = (bound, start);
                    continue;
                }
                return Some(match scope.imports.get(id) {
                    Some((module, level, imported)) => ConditionSource::Imported {
                        module: module.clone(),
                        level: *level,
                        name: (*imported).to_owned(),
                        position: position(scope.lines, value.range().start()),
                    },
                    None => {
                        unresolvable(format!("`{id}` is not bound at module level"), value, scope)
                    }
                });
            }
            other => {
                return Some(unresolvable(
                    format!(
                        "`{}` is not a condition call or a name",
                        excerpt(other, scope)
                    ),
                    value,
                    scope,
                ));
            }
        }
    }
}

/// A selecting call's `select`; nothing for a condition that selects no nodes; any other call cannot be followed.
fn call_source(call: &ExprCall, scope: &ModuleScope<'_>) -> Option<ConditionSource> {
    match authoring_call(call, (&scope.imported, &scope.modules)) {
        Some(name) if SELECTING_CONDITION_CALLS.contains(&name) => Some(
            ConditionSource::Selection(match call.arguments.find_keyword(SELECT_KEYWORD) {
                Some(select) => literal(&select.value, scope.lines),
                None => LiteralArgument {
                    value: None,
                    position: position(scope.lines, call.range().start()),
                },
            }),
        ),
        Some(_) => None,
        None => Some(unresolvable(
            format!("`{}(...)` is a function call", excerpt(&call.func, scope)),
            &call.func,
            scope,
        )),
    }
}

fn unresolvable(what: String, at: &Expr, scope: &ModuleScope<'_>) -> ConditionSource {
    ConditionSource::Unresolvable {
        what,
        position: position(scope.lines, at.range().start()),
    }
}

/// The first line of `expression`'s source, shortened to a quotable length.
fn excerpt(expression: &Expr, scope: &ModuleScope<'_>) -> String {
    let text: &str = scope.contents[expression.range()]
        .lines()
        .next()
        .unwrap_or_default();
    match text.char_indices().nth(EXCERPT_LIMIT) {
        Some((cut, _)) => format!("{}...", &text[..cut]),
        None => text.to_owned(),
    }
}

fn assignment(statement: &Stmt) -> Option<(&str, &Expr)> {
    match statement {
        Stmt::Assign(assign) => match assign.targets.as_slice() {
            [Expr::Name(target)] => Some((target.id.as_str(), assign.value.as_ref())),
            _ => None,
        },
        Stmt::AnnAssign(assign) => match (assign.target.as_ref(), assign.value.as_deref()) {
            (Expr::Name(target), Some(value)) => Some((target.id.as_str(), value)),
            _ => None,
        },
        _ => None,
    }
}
