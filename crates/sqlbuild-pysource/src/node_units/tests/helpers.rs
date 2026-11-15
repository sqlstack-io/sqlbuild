use crate::node_units::main::node_unit_module::node_unit_module;
use crate::node_units::models::{
    DeferredSpan, UnitDecorator, UnitImport, UnitParameter, UnitPosition, UnitStatementKind,
};

const PYTHON_VERSION: (u8, u8) = (3, 12);

/// The kind of each top-level statement of `contents` by variant name, or `None` when it does not parse.
pub(super) fn statement_kinds(contents: &str) -> Option<Vec<String>> {
    let module = node_unit_module(contents, PYTHON_VERSION)?;
    Some(
        module
            .statements
            .iter()
            .map(|statement| variant_name(&statement.kind))
            .collect(),
    )
}

/// The variant name of `kind`, such as `Function` for a function.
fn variant_name(kind: &UnitStatementKind) -> String {
    format!("{kind:?}")
        .split('(')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The imports `contents` binds at module level.
pub(super) fn imports(contents: &str) -> Vec<UnitImport> {
    node_unit_module(contents, PYTHON_VERSION).map_or_else(Vec::new, |module| module.imports)
}

/// What the last top-level statement of `contents` is.
pub(super) fn last_statement(contents: &str) -> Option<UnitStatementKind> {
    node_unit_module(contents, PYTHON_VERSION)?
        .statements
        .pop()
        .map(|statement| statement.kind)
}

pub(super) fn at(line: usize, column: usize) -> UnitPosition {
    UnitPosition { line, column }
}

pub(super) fn parameter(name: &str, annotation: Option<&str>) -> UnitParameter {
    UnitParameter {
        name: name.to_owned(),
        annotation: annotation.map(str::to_owned),
    }
}

pub(super) fn decorator(name: &str, keywords: &[&str], depends_on: &[&str]) -> UnitDecorator {
    UnitDecorator {
        name: name.to_owned(),
        keywords: keywords
            .iter()
            .map(|keyword| (*keyword).to_owned())
            .collect(),
        depends_on: depends_on.iter().map(|name| (*name).to_owned()).collect(),
    }
}

pub(super) fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

pub(super) fn import(
    module: &str,
    name: Option<&str>,
    local: &str,
    position: UnitPosition,
) -> UnitImport {
    UnitImport {
        module: module.to_owned(),
        level: 0,
        name: name.map(str::to_owned),
        local: local.to_owned(),
        position,
        only_in_depends_on: false,
    }
}

/// The span from `start` to `end`, each a 1-based line and 0-based byte column.
pub(super) fn span(start: (usize, usize), end: (usize, usize)) -> DeferredSpan {
    DeferredSpan { start, end }
}
