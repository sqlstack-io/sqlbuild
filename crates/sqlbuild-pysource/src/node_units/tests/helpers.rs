use crate::node_units::main::node_unit_module::node_unit_module;
use crate::node_units::models::{
    UnitDecorator, UnitFunction, UnitImport, UnitParameter, UnitPosition, UnitStatementKind,
};

const PYTHON_VERSION: (u8, u8) = (3, 12);

/// The kind of each top-level statement of `contents`, or `None` when it does not parse.
pub(super) fn statement_kinds(contents: &str) -> Option<Vec<&'static str>> {
    let module = node_unit_module(contents, PYTHON_VERSION)?;
    Some(
        module
            .statements
            .iter()
            .map(|statement| match statement.kind {
                UnitStatementKind::Docstring => "docstring",
                UnitStatementKind::Import => "import",
                UnitStatementKind::Function(_) => "function",
                UnitStatementKind::Other => "other",
            })
            .collect(),
    )
}

/// The imports `contents` binds at module level.
pub(super) fn imports(contents: &str) -> Vec<UnitImport> {
    node_unit_module(contents, PYTHON_VERSION).map_or_else(Vec::new, |module| module.imports)
}

/// The first top-level function of `contents`.
pub(super) fn first_function(contents: &str) -> Option<UnitFunction> {
    node_unit_module(contents, PYTHON_VERSION)?
        .statements
        .into_iter()
        .find_map(|statement| match statement.kind {
            UnitStatementKind::Function(function) => Some(function),
            _ => None,
        })
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
