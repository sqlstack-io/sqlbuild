//! Sensor and calendar modules under orchestration/: where they sit, what they hold and what they import.

use std::collections::BTreeSet;

use sqlbuild_pysource::node_units::models::{UnitPosition, UnitStatementKind};
use sqlbuild_pysource::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::_helpers::units::project::{
    files_naming, module_below, nearest_unit, package_rest, project_modules,
    rejected_import_faults, unit_fault, unit_folders,
};
use crate::python_rules::constants::{
    INIT_FILE, ORCHESTRATION_GENERIC_MODULES, ORCHESTRATION_IMPORT_DIRECTION_CODE,
    ORCHESTRATION_MODULE_PATHS_CODE, ORCHESTRATION_MODULE_SURFACE_CODE,
    ORCHESTRATION_NO_DOCSTRING_CODE, ORCHESTRATION_PRIVATE_SUPPORT_CODE,
    ORCHESTRATION_PYTHON_ROLES, ORCHESTRATION_ROOT, ORCHESTRATION_RULE_CODES,
    ORCHESTRATION_SUPPORT_FILES, ORCHESTRATION_SUPPORT_FOLDER, PACKAGE_MODULE, PACKAGE_ROOT, PRIVATE_PREFIX,
    PROVIDERS_ROOT, SHARED_UTILITIES_PACKAGE,
};
use crate::python_rules::models::ParsedFile;

const OTHER_STATEMENT: &str =
    "a statement that is not an import, a private constant or the declaration";

/// Faults of the selected orchestration rules among `codes`.
pub(crate) fn orchestration_faults(
    sources: &[PythonSource],
    codes: &BTreeSet<&str>,
    python_version: (u8, u8),
) -> Vec<Fault> {
    if !ORCHESTRATION_RULE_CODES
        .iter()
        .any(|code| codes.contains(code))
    {
        return Vec::new();
    }
    let mut faults: Vec<Fault> = sources.iter().filter_map(layout_fault).collect();
    let files: Vec<ParsedFile<'_>> = files_naming(sources, (ORCHESTRATION_ROOT, ORCHESTRATION_ROOT), python_version);
    let modules: BTreeSet<String> = project_modules(sources);
    let units: BTreeSet<String> = unit_folders(sources);
    for file in &files {
        if role_folders(&file.parts).is_some_and(|rest| !is_private(rest)) {
            faults.extend(surface_faults(file, python_version));
        }
        faults.extend(import_faults(file, &modules, &units));
    }
    faults.retain(|fault| codes.contains(fault.code.as_str()));
    faults
}

/// The path parts below `orchestration/<role>/` for a sensor or calendar file; `None` for any other file.
fn role_folders<'p>(parts: &'p [&'p str]) -> Option<&'p [&'p str]> {
    match parts {
        [root, role, rest @ ..]
            if *root == ORCHESTRATION_ROOT && ORCHESTRATION_PYTHON_ROLES.contains(role) =>
        {
            Some(rest)
        }
        _ => None,
    }
}

/// Whether a file below a role folder is private support: its name or a folder above it starts with `_`.
fn is_private(rest: &[&str]) -> bool {
    rest.iter().any(|part| part.starts_with(PRIVATE_PREFIX))
}

/// Where a sensor or calendar file sits wrongly: outside a domain folder, nested, generic, or odd support.
fn layout_fault(source: &PythonSource) -> Option<Fault> {
    let path: &str = source.relative_path.as_str();
    let parts: Vec<&str> = path.split('/').collect();
    let rest: &[&str] = role_folders(&parts)?;
    let role: String = format!("{ORCHESTRATION_ROOT}/{}", parts[1]);
    let (code, detail): (&str, String) = match rest {
        [file] if *file == INIT_FILE => return None,
        [file] if file.starts_with(PRIVATE_PREFIX) => (
            ORCHESTRATION_PRIVATE_SUPPORT_CODE,
            format!("{path} is private support outside a domain folder of {role}/"),
        ),
        [_] => (
            ORCHESTRATION_MODULE_PATHS_CODE,
            format!("{path} sits directly in {role}/, not in a domain folder"),
        ),
        [domain, ..] if domain.starts_with(PRIVATE_PREFIX) => (
            ORCHESTRATION_PRIVATE_SUPPORT_CODE,
            format!("{path} is in private folder {domain}/ outside a domain folder of {role}/"),
        ),
        [_, file] if *file == INIT_FILE => return None,
        [_, file] if file.starts_with(PRIVATE_PREFIX) => {
            if ORCHESTRATION_SUPPORT_FILES.contains(file) {
                return None;
            }
            (
                ORCHESTRATION_PRIVATE_SUPPORT_CODE,
                format!("{path} is not _models.py, _types.py, _constants.py or _exceptions.py"),
            )
        }
        [_, file] => {
            if !ORCHESTRATION_GENERIC_MODULES.contains(file) {
                return None;
            }
            (
                ORCHESTRATION_MODULE_PATHS_CODE,
                format!("{file} is too generic; name the module for its capability"),
            )
        }
        [_, folder, ..] if *folder == ORCHESTRATION_SUPPORT_FOLDER => return None,
        [domain, folder, ..] if folder.starts_with(PRIVATE_PREFIX) => (
            ORCHESTRATION_PRIVATE_SUPPORT_CODE,
            format!("{path} is in {folder}/, but {domain}/'s only private folder is _helpers/"),
        ),
        [domain, ..] => (
            ORCHESTRATION_MODULE_PATHS_CODE,
            format!("{path} is nested below its domain folder {domain}/"),
        ),
        [] => return None,
    };
    catalogue_fault(code, path, (1, 1), &detail)
}

/// A declaration module's statements beyond imports, private constants and the declaration, and its docstring.
fn surface_faults(file: &ParsedFile<'_>, python_version: (u8, u8)) -> Vec<Fault> {
    let declaration_lines: Vec<usize> =
        orchestration_module_declarations(file.text, python_version)
            .map(|module| module.declarations)
            .unwrap_or_default()
            .iter()
            .map(|declaration| declaration.position.line)
            .collect();
    let statements = &file.module.statements;
    let mut faults: Vec<Fault> = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        let end: usize = statements
            .get(index + 1)
            .map_or(usize::MAX, |next| next.position.line);
        let declares: bool = declaration_lines
            .iter()
            .any(|line| (statement.position.line..end).contains(line));
        let (code, detail, position): (&str, String, UnitPosition) = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Function(function) if declares && function.docstring => (
                ORCHESTRATION_NO_DOCSTRING_CODE,
                format!(
                    "'{}' has a docstring; its description is description=",
                    function.name
                ),
                function.position,
            ),
            _ if declares => continue,
            UnitStatementKind::Assignment(names) => match public_name(names) {
                Some(detail) => (
                    ORCHESTRATION_MODULE_SURFACE_CODE,
                    detail,
                    statement.position,
                ),
                None => continue,
            },
            UnitStatementKind::Function(function) => (
                ORCHESTRATION_MODULE_SURFACE_CODE,
                format!("function '{}' belongs in _helpers/", function.name),
                function.position,
            ),
            UnitStatementKind::Class(class) => (
                ORCHESTRATION_MODULE_SURFACE_CODE,
                format!("class '{}' belongs in _models.py or _helpers/", class.name),
                class.position,
            ),
            UnitStatementKind::Other => (
                ORCHESTRATION_MODULE_SURFACE_CODE,
                OTHER_STATEMENT.to_owned(),
                statement.position,
            ),
        };
        faults.extend(unit_fault(code, file.path, position, &detail));
    }
    faults
}

/// Why an assignment does not belong in a declaration module; `None` for a private constant.
fn public_name(names: &[String]) -> Option<String> {
    if names.is_empty() {
        return Some(OTHER_STATEMENT.to_owned());
    }
    let name: &String = names
        .iter()
        .find(|name| !name.starts_with(PRIVATE_PREFIX))?;
    Some(format!("public constant '{name}' belongs in _constants.py"))
}

/// Imports of another domain's private support, and imports across the orchestration boundary.
fn import_faults(
    file: &ParsedFile<'_>,
    modules: &BTreeSet<String>,
    units: &BTreeSet<String>,
) -> Vec<Fault> {
    rejected_import_faults(file, modules, |module| {
        boundary_detail(&file.parts, module, units)
    })
}

/// Why a file at `parts` may not import `module`; node units and providers are left to 008 and 015.
fn boundary_detail(
    parts: &[&str],
    module: &str,
    units: &BTreeSet<String>,
) -> Option<(&'static str, String)> {
    let imported: Vec<&str> = module.split('.').collect();
    let path: String = parts.join("/");
    let Some(rest) = role_folders(parts) else {
        let in_unit: bool = package_rest(&path).is_some_and(|below| {
            let below: Vec<&str> = below.split('/').collect();
            nearest_unit(&below, units).is_some()
        });
        let in_providers: bool = path
            .strip_prefix(PROVIDERS_ROOT)
            .is_some_and(|below| below.starts_with('/'));
        let outside: bool = parts.first() != Some(&ORCHESTRATION_ROOT)
            && !in_providers
            && !in_unit
            && imported.first() == Some(&ORCHESTRATION_ROOT);
        return outside.then(|| {
            (
                ORCHESTRATION_IMPORT_DIRECTION_CODE,
                format!("but only {ORCHESTRATION_ROOT}/ imports from {ORCHESTRATION_ROOT}/"),
            )
        });
    };
    if let Some(below) = module_below(module, PACKAGE_MODULE) {
        return (below.first() != Some(&SHARED_UTILITIES_PACKAGE)).then(|| {
            (
                ORCHESTRATION_IMPORT_DIRECTION_CODE,
                format!(
                    "but sensors and calendars import only {PACKAGE_ROOT}/{SHARED_UTILITIES_PACKAGE}/ from {PACKAGE_ROOT}/"
                ),
            )
        });
    }
    match imported.as_slice() {
        [root, role, domain, support @ ..]
            if *root == ORCHESTRATION_ROOT
                && is_private(support)
                && (Some(role) != parts.get(1) || Some(domain) != rest.first()) =>
        {
            Some((
                ORCHESTRATION_PRIVATE_SUPPORT_CODE,
                format!("which is private to {ORCHESTRATION_ROOT}/{role}/{domain}/"),
            ))
        }
        _ => None,
    }
}
