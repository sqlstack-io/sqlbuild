//! A sensor condition's selection, following names bound at module level and noting names imported from elsewhere.

use std::collections::BTreeMap;

use ruff_python_ast::{Expr, ExprCall, Stmt};
use ruff_text_size::{Ranged, TextSize};

use crate::orchestration_modules::_helpers::calls::{authoring_call, literal, position};
use crate::orchestration_modules::constants::{
    EXCERPT_LIMIT, SELECT_KEYWORD, SELECTING_CONDITION_CALLS,
};
use crate::orchestration_modules::models::{
    ConditionSource, ImportedName, LiteralArgument, ModuleScope,
};

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

/// Each name's module-level `from ... import` bindings, in source order.
pub(crate) fn module_imports(body: &[Stmt]) -> BTreeMap<&str, Vec<ImportedName<'_>>> {
    let mut imports: BTreeMap<&str, Vec<ImportedName<'_>>> = BTreeMap::new();
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
            imports.entry(local).or_default().push(ImportedName {
                module: module.clone(),
                level: import.level,
                name: alias.name.as_str(),
                start: statement.range().start(),
            });
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
                let assigned = scope.assignments.get(id).and_then(|bindings| {
                    bindings
                        .iter()
                        .rev()
                        .find(|(start, _)| *start < at)
                        .copied()
                });
                let imported = scope
                    .imports
                    .get(id)
                    .and_then(|bindings| bindings.iter().rev().find(|import| import.start < at));
                match (assigned, imported) {
                    (Some((start, bound)), imported)
                        if imported.is_none_or(|import| import.start < start) =>
                    {
                        current = (bound, start);
                    }
                    (_, Some(import)) => return Some(imported_source(import, value, scope)),
                    _ => {
                        return Some(unresolvable(
                            format!("`{id}` is not bound at module level"),
                            value,
                            scope,
                        ));
                    }
                }
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

/// The imported name `import` binds, reported where `at` uses it.
pub(crate) fn imported_source(
    import: &ImportedName<'_>,
    at: &Expr,
    scope: &ModuleScope<'_>,
) -> ConditionSource {
    ConditionSource::Imported {
        module: import.module.clone(),
        level: import.level,
        name: import.name.to_owned(),
        position: position(scope.lines, at.range().start()),
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
    let text: &str = scope
        .contents
        .get(std::ops::Range::<usize>::from(expression.range()))
        .unwrap_or_default()
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
