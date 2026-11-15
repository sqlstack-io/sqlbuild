//! The statements a unit's node.py, main.py and checks.py may hold.

use sqlbuild_pysource::node_units::models::{UnitFunction, UnitPosition, UnitStatementKind};

use crate::models::Fault;
use crate::python_rules::_helpers::units::project::{
    Declared, declared, names_from_unit_modules, top_functions, unit_fault, unit_file_name,
    unit_member,
};
use crate::python_rules::constants::{
    CHECKS_MODULE_SHAPE_CODE, MAIN_MODULE_SHAPE_CODE, MAIN_NAME, MAIN_RESULTS, NODE_MODULE,
    NODE_MODULE_SHAPE_CODE, UNIT_CHECKS_FILE, UNIT_MAIN_FILE, UNIT_NODE_FILE,
};
use crate::python_rules::models::{UnitFile, UnitProject};

const FILE_START: UnitPosition = UnitPosition { line: 1, column: 1 };
const OTHER_STATEMENT: &str = "a statement that is not an import or a function";

/// Statements and functions node.py, main.py and checks.py may not hold.
pub(crate) fn module_shape_faults(project: &UnitProject<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for file in &project.files {
        match unit_file_name(file) {
            Some(UNIT_NODE_FILE) => faults.extend(node_module_faults(file)),
            Some(UNIT_MAIN_FILE) => faults.extend(main_module_faults(file, project)),
            Some(UNIT_CHECKS_FILE) => faults.extend(checks_module_faults(file, project)),
            _ => {}
        }
    }
    faults
}

/// node.py: imports and exactly one node or factory declaration.
fn node_module_faults(file: &UnitFile<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut declarations: usize = 0;
    for statement in &file.module.statements {
        let (detail, position): (String, UnitPosition) = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Function(function) => match declared(function) {
                Some((Declared::Node(..) | Declared::Factory, _)) => {
                    declarations += 1;
                    if declarations == 1 {
                        continue;
                    }
                    (
                        format!("'{}' is a second declaration", function.name),
                        function.position,
                    )
                }
                _ => (
                    format!("function '{}' is not the node declaration", function.name),
                    function.position,
                ),
            },
            UnitStatementKind::Class(_)
            | UnitStatementKind::Assignment(_)
            | UnitStatementKind::Other => (OTHER_STATEMENT.to_owned(), statement.position),
        };
        faults.extend(unit_fault(
            NODE_MODULE_SHAPE_CODE,
            file.path,
            position,
            &detail,
        ));
    }
    if declarations == 0 {
        faults.extend(unit_fault(
            NODE_MODULE_SHAPE_CODE,
            file.path,
            FILE_START,
            &format!("{} declares no node", file.path),
        ));
    }
    faults
}

/// main.py: imports and one public main() whose return annotation fits the unit's node kind.
fn main_module_faults(file: &UnitFile<'_>, project: &UnitProject<'_>) -> Vec<Fault> {
    let kind: Option<&str> = file
        .unit
        .as_deref()
        .and_then(|unit| node_kind(project, unit));
    let mut faults: Vec<Fault> = Vec::new();
    let mut found: bool = false;
    for statement in &file.module.statements {
        let (detail, position): (String, UnitPosition) = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Function(function) if function.name == MAIN_NAME && !found => {
                found = true;
                match main_result_detail(function, kind) {
                    Some(detail) => (detail, function.position),
                    None => continue,
                }
            }
            UnitStatementKind::Function(function) => (
                format!("function '{}' is not main()", function.name),
                function.position,
            ),
            UnitStatementKind::Class(_)
            | UnitStatementKind::Assignment(_)
            | UnitStatementKind::Other => (OTHER_STATEMENT.to_owned(), statement.position),
        };
        faults.extend(unit_fault(
            MAIN_MODULE_SHAPE_CODE,
            file.path,
            position,
            &detail,
        ));
    }
    if !found {
        faults.extend(unit_fault(
            MAIN_MODULE_SHAPE_CODE,
            file.path,
            FILE_START,
            &format!("{} defines no main()", file.path),
        ));
    }
    faults
}

/// Why main()'s return annotation does not fit a node of `kind`, if it does not.
fn main_result_detail(function: &UnitFunction, kind: Option<&str>) -> Option<String> {
    let Some(returns) = &function.returns else {
        return Some("main() has no return annotation".to_owned());
    };
    let kind: &str = kind?;
    let (_, allowed) = MAIN_RESULTS.iter().find(|(name, _)| *name == kind)?;
    let wrong: Vec<&str> = returns
        .iter()
        .map(String::as_str)
        .filter(|name| !allowed.contains(name))
        .collect();
    (!wrong.is_empty()).then(|| {
        format!(
            "main() returns {}, which is not a @{kind} result",
            wrong.join(" | ")
        )
    })
}

/// The kind of the node the unit's node.py declares, or of the first node its factory generates.
fn node_kind(project: &UnitProject<'_>, unit: &str) -> Option<&'static str> {
    let node: &UnitFile<'_> = unit_member(project, unit, UNIT_NODE_FILE)?;
    top_functions(node).find_map(|function| match declared(function)? {
        (Declared::Node(kind, _), _) => Some(kind),
        (Declared::Factory, _) => function.inner.iter().find_map(declared_node_kind),
        _ => None,
    })
}

/// The kind of node `function` declares, if it declares one.
fn declared_node_kind(function: &UnitFunction) -> Option<&'static str> {
    match declared(function)? {
        (Declared::Node(kind, _), _) => Some(kind),
        _ => None,
    }
}

/// checks.py: imports and checks of the unit's own node that return directly, or one factory.
fn checks_module_faults(file: &UnitFile<'_>, project: &UnitProject<'_>) -> Vec<Fault> {
    let unit_node: Vec<String> = file.unit.as_deref().map_or_else(Vec::new, |unit| {
        let mut names: Vec<String> = names_from_unit_modules(file, project, unit, &[NODE_MODULE])
            .into_iter()
            .collect();
        if let Some(node) = unit_member(project, unit, UNIT_NODE_FILE) {
            names.extend(top_functions(node).map(|function| function.name.clone()));
        }
        names
    });
    let mut faults: Vec<Fault> = Vec::new();
    let mut factories: usize = 0;
    for statement in &file.module.statements {
        let details: Vec<(String, UnitPosition)> = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Function(function) => match declared(function) {
                Some((Declared::Check, decorator)) => {
                    let mut details: Vec<(String, UnitPosition)> = Vec::new();
                    if !decorator
                        .depends_on
                        .iter()
                        .any(|name| unit_node.contains(name))
                    {
                        details.push((
                            format!(
                                "check '{}' does not depend on its unit's node",
                                function.name
                            ),
                            function.position,
                        ));
                    }
                    if !function.returns_directly {
                        details.push((
                            format!("check '{}' does more than return", function.name),
                            function.position,
                        ));
                    }
                    details
                }
                Some((Declared::Factory, _)) => {
                    factories += 1;
                    if factories == 1 {
                        continue;
                    }
                    vec![(
                        format!("'{}' is a second factory", function.name),
                        function.position,
                    )]
                }
                _ => vec![(
                    format!("function '{}' is not a check", function.name),
                    function.position,
                )],
            },
            UnitStatementKind::Class(_)
            | UnitStatementKind::Assignment(_)
            | UnitStatementKind::Other => {
                vec![(OTHER_STATEMENT.to_owned(), statement.position)]
            }
        };
        faults.extend(details.iter().filter_map(|(detail, position)| {
            unit_fault(CHECKS_MODULE_SHAPE_CODE, file.path, *position, detail)
        }));
    }
    faults
}
