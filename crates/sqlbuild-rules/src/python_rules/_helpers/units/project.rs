//! The project's node units, read from parsed `python/` modules, and the unit rules run over them.

use std::collections::BTreeSet;

use sqlbuild_pysource::node_units::main::node_unit_module::node_unit_module;
use sqlbuild_pysource::node_units::models::{
    UnitDecorator, UnitFunction, UnitImport, UnitPosition, UnitStatementKind,
};

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::faults::catalogue_fault;
use crate::python_rules::_helpers::units::imports::import_boundary_faults;
use crate::python_rules::_helpers::units::layout::{
    declared_outside_unit_faults, unit_layout_faults,
};
use crate::python_rules::_helpers::units::modules::module_shape_faults;
use crate::python_rules::_helpers::units::nodes::declaration_faults;
use crate::python_rules::constants::{
    CHECK_KIND, FACTORY_DECORATORS, INIT_FILE, NODE_KINDS, PACKAGE_ROOT, UNIT_NODE_FILE,
    UNIT_RULE_CODES,
};
use crate::python_rules::models::{UnitFile, UnitProject};

/// Faults of the selected unit rules among `codes`.
pub(crate) fn unit_faults(
    sources: &[PythonSource],
    codes: &BTreeSet<&str>,
    python_version: (u8, u8),
) -> Vec<Fault> {
    if !UNIT_RULE_CODES.iter().any(|code| codes.contains(code)) {
        return Vec::new();
    }
    let project: UnitProject<'_> = unit_project(sources, python_version);
    let mut faults: Vec<Fault> = unit_layout_faults(&project, sources);
    faults.extend(declared_outside_unit_faults(&project));
    faults.extend(module_shape_faults(&project));
    faults.extend(declaration_faults(&project));
    faults.extend(import_boundary_faults(&project));
    faults.retain(|fault| codes.contains(fault.code.as_str()));
    faults
}

fn unit_project(sources: &[PythonSource], python_version: (u8, u8)) -> UnitProject<'_> {
    let package: Vec<(&PythonSource, Vec<&str>)> = sources
        .iter()
        .filter_map(|source| {
            let (root, rest) = source.relative_path.split_once('/')?;
            (root == PACKAGE_ROOT).then(|| (source, rest.split('/').collect()))
        })
        .collect();
    let units: BTreeSet<String> = package
        .iter()
        .filter(
            |(_, parts)| matches!(parts.as_slice(), [_, _, .., last] if *last == UNIT_NODE_FILE),
        )
        .map(|(_, parts)| parts[..parts.len() - 1].join("/"))
        .collect();
    let files: Vec<UnitFile<'_>> = package
        .iter()
        .filter_map(|(source, parts)| {
            Some(UnitFile {
                path: source.relative_path.as_str(),
                module: node_unit_module(&source.text, python_version)?,
                unit: nearest_unit(parts, &units),
                parts: parts.clone(),
            })
        })
        .collect();
    UnitProject {
        files,
        units,
        paths: sources
            .iter()
            .map(|source| source.relative_path.as_str())
            .collect(),
        modules: sources
            .iter()
            .flat_map(|source| dotted_names(&source.relative_path))
            .collect(),
    }
}

/// The deepest unit folder among the file's folders.
fn nearest_unit(parts: &[&str], units: &BTreeSet<String>) -> Option<String> {
    (1..parts.len())
        .rev()
        .map(|depth| parts[..depth].join("/"))
        .find(|folder| units.contains(folder))
}

/// The dotted module of a Python path and of every package above it.
fn dotted_names(path: &str) -> Vec<String> {
    let stem: &str = path.strip_suffix(".py").unwrap_or(path);
    let parts: Vec<&str> = stem
        .split('/')
        .filter(|part| *part != INIT_FILE.trim_end_matches(".py"))
        .collect();
    (1..=parts.len())
        .map(|depth| parts[..depth].join("."))
        .collect()
}

/// A fault of rule `code` at `position` in `path`.
pub(crate) fn unit_fault(
    code: &str,
    path: &str,
    position: UnitPosition,
    detail: &str,
) -> Option<Fault> {
    catalogue_fault(
        code,
        path,
        (position.line as u64, position.column as u64),
        detail,
    )
}

/// The file's position in its unit: the file name when it sits directly in the unit folder.
pub(crate) fn unit_file_name<'a>(file: &UnitFile<'a>) -> Option<&'a str> {
    let unit: &str = file.unit.as_deref()?;
    let depth: usize = unit.split('/').count();
    (file.parts.len() == depth + 1).then(|| file.parts[depth])
}

/// The file in `project` at `python/<unit>/<name>`.
pub(crate) fn unit_member<'p, 'a>(
    project: &'p UnitProject<'a>,
    unit: &str,
    name: &str,
) -> Option<&'p UnitFile<'a>> {
    let path: String = format!("{PACKAGE_ROOT}/{unit}/{name}");
    project.files.iter().find(|file| file.path == path)
}

/// What a decorator declares: a node kind, `check`, or a factory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Declared {
    Node(&'static str, &'static str),
    Check,
    Factory,
}

/// What `function`'s decorators declare, if anything.
pub(crate) fn declared(function: &UnitFunction) -> Option<(Declared, &UnitDecorator)> {
    function
        .decorators
        .iter()
        .find_map(|decorator| decorator_declares(&decorator.name).map(|what| (what, decorator)))
}

/// What a decorator named `name` declares, if anything.
fn decorator_declares(name: &str) -> Option<Declared> {
    if let Some((kind, context)) = NODE_KINDS.iter().find(|(kind, _)| *kind == name) {
        return Some(Declared::Node(kind, context));
    }
    if name == CHECK_KIND.0 {
        return Some(Declared::Check);
    }
    FACTORY_DECORATORS
        .contains(&name)
        .then_some(Declared::Factory)
}

/// The module's top-level functions.
pub(crate) fn top_functions<'f>(file: &'f UnitFile<'_>) -> impl Iterator<Item = &'f UnitFunction> {
    file.module
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            UnitStatementKind::Function(function) => Some(function),
            _ => None,
        })
}

/// The absolute dotted module `import` reads, resolving relative levels and `from package import module`.
pub(crate) fn imported_module(
    import: &UnitImport,
    file: &UnitFile<'_>,
    project: &UnitProject<'_>,
) -> Option<String> {
    let mut base: Vec<&str> = Vec::new();
    if import.level > 0 {
        base.push(PACKAGE_ROOT);
        base.extend(&file.parts[..file.parts.len() - 1]);
        for _ in 1..import.level {
            base.pop()?;
        }
    }
    base.extend(import.module.split('.').filter(|part| !part.is_empty()));
    let module: String = base.join(".");
    let Some(name) = &import.name else {
        return Some(module);
    };
    let submodule: String = format!("{module}.{name}");
    Some(if project.modules.contains(&submodule) {
        submodule
    } else {
        module
    })
}

/// The names `file` binds by importing from `python.<unit>.<module>`, one of `modules`.
pub(crate) fn names_from_unit_modules(
    file: &UnitFile<'_>,
    project: &UnitProject<'_>,
    unit: &str,
    modules: &[&str],
) -> BTreeSet<String> {
    let targets: Vec<String> = modules
        .iter()
        .map(|module| format!("{PACKAGE_ROOT}.{}.{module}", unit.replace('/', ".")))
        .collect();
    file.module
        .imports
        .iter()
        .filter(|import| import.name.is_some())
        .filter(|import| {
            imported_module(import, file, project).is_some_and(|module| targets.contains(&module))
        })
        .map(|import| import.local.clone())
        .collect()
}
