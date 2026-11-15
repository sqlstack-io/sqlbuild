//! Provider capability folders: what they hold, what provider.py declares, and who imports their private code.

use std::collections::BTreeSet;

use sqlbuild_pysource::node_units::main::node_unit_module::node_unit_module;
use sqlbuild_pysource::node_units::models::{
    UnitClass, UnitModule, UnitPosition, UnitStatementKind,
};

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::_helpers::engine::placement::package_anchor;
use crate::python_rules::_helpers::units::project::{absolute_module, dotted_names, unit_fault};
use crate::python_rules::constants::{
    INIT_FILE, ORCHESTRATION_ROOT, PACKAGE_ROOT, PRIVATE_PREFIX, PROVIDER_BASE, PROVIDER_FILE,
    PROVIDER_IMPORT_BOUNDARIES_CODE, PROVIDER_LAYOUT_CODE, PROVIDER_MODULE_SHAPE_CODE,
    PROVIDER_PRIVATE_FOLDERS, PROVIDER_ROLE_FILES, PROVIDER_RULE_CODES, PROVIDERS_ROOT,
    SHARED_UTILITIES_PACKAGE,
};

const FILE_START: UnitPosition = UnitPosition { line: 1, column: 1 };

/// One parsed project file and its path parts.
#[derive(Debug)]
struct ParsedFile<'a> {
    path: &'a str,
    parts: Vec<&'a str>,
    module: UnitModule,
}

/// Faults of the selected provider rules among `codes`.
pub(crate) fn provider_faults(
    sources: &[PythonSource],
    codes: &BTreeSet<&str>,
    python_version: (u8, u8),
) -> Vec<Fault> {
    if !PROVIDER_RULE_CODES.iter().any(|code| codes.contains(code)) {
        return Vec::new();
    }
    let mut faults: Vec<Fault> = provider_layout_faults(sources);
    let files: Vec<ParsedFile<'_>> = sources
        .iter()
        .filter_map(|source| {
            Some(ParsedFile {
                path: source.relative_path.as_str(),
                parts: source.relative_path.split('/').collect(),
                module: node_unit_module(&source.text, python_version)?,
            })
        })
        .collect();
    let modules: BTreeSet<String> = sources
        .iter()
        .flat_map(|source| dotted_names(&source.relative_path))
        .collect();
    for file in &files {
        if matches!(file.parts.as_slice(), [root, _, file_name] if *root == PROVIDERS_ROOT && *file_name == PROVIDER_FILE)
        {
            faults.extend(provider_module_faults(file));
        }
        faults.extend(import_faults(file, &modules));
    }
    faults.retain(|fault| codes.contains(fault.code.as_str()));
    faults
}

/// Modules directly in providers/, capability folders without provider.py, and modules or folders they may not hold.
fn provider_layout_faults(sources: &[PythonSource]) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut capabilities: BTreeSet<&str> = BTreeSet::new();
    let mut folders: BTreeSet<String> = BTreeSet::new();
    for source in sources {
        let path: &str = source.relative_path.as_str();
        let Some(rest) = path.strip_prefix(&format!("{PROVIDERS_ROOT}/")) else {
            continue;
        };
        let parts: Vec<&str> = rest.split('/').collect();
        let detail: Option<String> = match parts.as_slice() {
            [file] if *file == INIT_FILE => None,
            [_] => Some(format!("{path} sits directly in {PROVIDERS_ROOT}/")),
            [capability, file] => {
                capabilities.insert(capability);
                (*file != PROVIDER_FILE && !PROVIDER_ROLE_FILES.contains(file))
                    .then(|| format!("{path} is not provider.py or a provider role file"))
            }
            [capability, folder, ..] => {
                capabilities.insert(capability);
                if !PROVIDER_PRIVATE_FOLDERS.contains(folder) {
                    folders.insert(format!("{PROVIDERS_ROOT}/{capability}/{folder}"));
                }
                None
            }
            [] => None,
        };
        if let Some(detail) = detail {
            faults.extend(catalogue_fault(PROVIDER_LAYOUT_CODE, path, (1, 1), &detail));
        }
    }
    let paths: BTreeSet<&str> = sources
        .iter()
        .map(|source| source.relative_path.as_str())
        .collect();
    faults.extend(
        capabilities
            .iter()
            .map(|capability| format!("{PROVIDERS_ROOT}/{capability}"))
            .filter(|folder| !paths.contains(format!("{folder}/{PROVIDER_FILE}").as_str()))
            .filter_map(|folder| {
                catalogue_fault(
                    PROVIDER_LAYOUT_CODE,
                    package_anchor(sources, &folder)?,
                    (1, 1),
                    &format!("{folder}/ has no {PROVIDER_FILE}"),
                )
            }),
    );
    faults.extend(folders.iter().filter_map(|folder| {
        catalogue_fault(
            PROVIDER_LAYOUT_CODE,
            package_anchor(sources, folder)?,
            (1, 1),
            &format!("{folder}/ is not _helpers/ or _classes/"),
        )
    }));
    faults
}

/// provider.py: imports, private constants and exactly one public Provider subclass.
fn provider_module_faults(file: &ParsedFile<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut providers: usize = 0;
    for statement in &file.module.statements {
        let (detail, position): (String, UnitPosition) = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Assignment(names) => {
                match names.iter().find(|name| !name.starts_with(PRIVATE_PREFIX)) {
                    Some(name) => (
                        format!("public constant '{name}' belongs in constants.py"),
                        statement.position,
                    ),
                    None => continue,
                }
            }
            UnitStatementKind::Class(class) => match class_detail(class, &mut providers) {
                Some(detail) => (detail, class.position),
                None => continue,
            },
            UnitStatementKind::Function(function) => (
                format!("function '{}' belongs in _helpers/", function.name),
                function.position,
            ),
            UnitStatementKind::Other => (
                "a statement that is not an import, a constant or the provider class".to_owned(),
                statement.position,
            ),
        };
        faults.extend(unit_fault(
            PROVIDER_MODULE_SHAPE_CODE,
            file.path,
            position,
            &detail,
        ));
    }
    if providers == 0 {
        faults.extend(unit_fault(
            PROVIDER_MODULE_SHAPE_CODE,
            file.path,
            FILE_START,
            &format!("{} declares no public {PROVIDER_BASE} subclass", file.path),
        ));
    }
    faults
}

/// Why `class` does not belong in provider.py, counting public Provider subclasses in `providers`.
fn class_detail(class: &UnitClass, providers: &mut usize) -> Option<String> {
    let name: &str = &class.name;
    if name.starts_with(PRIVATE_PREFIX) {
        return Some(format!("collaborator class '{name}' belongs in _classes/"));
    }
    if !class.bases.iter().any(|base| base == PROVIDER_BASE) {
        return Some(format!("class '{name}' does not subclass {PROVIDER_BASE}"));
    }
    *providers += 1;
    (*providers > 1).then(|| format!("'{name}' is a second {PROVIDER_BASE} subclass"))
}

/// Imports of another provider's private code, and a provider's imports of python/ outside utils/ or of orchestration/.
fn import_faults(file: &ParsedFile<'_>, modules: &BTreeSet<String>) -> Vec<Fault> {
    let capability: Option<&str> = match file.parts.as_slice() {
        [root, capability, _, ..] if *root == PROVIDERS_ROOT => Some(capability),
        _ => None,
    };
    let folder: &[&str] = &file.parts[..file.parts.len() - 1];
    file.module
        .imports
        .iter()
        .filter_map(|import| {
            let module: String = absolute_module(import, folder, modules)?;
            if !modules.contains(&module) {
                return None;
            }
            let detail: String = boundary_detail(&module, capability)?;
            unit_fault(
                PROVIDER_IMPORT_BOUNDARIES_CODE,
                file.path,
                import.position,
                &format!("{} imports {module}, {detail}", file.path),
            )
        })
        .collect()
}

/// Why a file in provider `capability` (or outside providers/ when `None`) may not import `module`.
fn boundary_detail(module: &str, capability: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = module.split('.').collect();
    if let [root, owner, folder, ..] = parts.as_slice() {
        let private: bool = *root == PROVIDERS_ROOT && PROVIDER_PRIVATE_FOLDERS.contains(folder);
        if private && capability != Some(*owner) {
            return Some(format!("which is private to {PROVIDERS_ROOT}/{owner}/"));
        }
    }
    capability?;
    match parts.as_slice() {
        [root, utils, ..] if *root == PACKAGE_ROOT && *utils == SHARED_UTILITIES_PACKAGE => None,
        [root, ..] if *root == PACKAGE_ROOT => Some(format!(
            "but a provider imports only {PACKAGE_ROOT}/{SHARED_UTILITIES_PACKAGE}/ from {PACKAGE_ROOT}/"
        )),
        [root, ..] if *root == ORCHESTRATION_ROOT => Some(format!(
            "but a provider never imports {ORCHESTRATION_ROOT}/"
        )),
        _ => None,
    }
}
