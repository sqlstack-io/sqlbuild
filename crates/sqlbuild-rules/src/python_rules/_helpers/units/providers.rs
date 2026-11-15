//! Provider capability folders: what they hold, what provider.py declares, and who imports their private code.

use std::collections::{BTreeMap, BTreeSet};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

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
    SHARED_UTILITIES_PACKAGE, STAR_IMPORT,
};

const FILE_START: UnitPosition = UnitPosition { line: 1, column: 1 };
const OTHER_STATEMENT: &str = "a statement that is not an import, a constant or the provider class";
/// The package Provider is imported from.
const SQLBUILD_PACKAGE: &str = "sqlbuild";

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
        .par_iter()
        .filter(|source| {
            source.relative_path.split('/').next() == Some(PROVIDERS_ROOT)
                || source.text.contains(PROVIDERS_ROOT)
        })
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
    let classes: ProviderClasses = provider_classes(&files, &modules);
    for file in &files {
        if matches!(file.parts.as_slice(), [root, _, file_name] if *root == PROVIDERS_ROOT && *file_name == PROVIDER_FILE)
        {
            faults.extend(provider_module_faults(file, &classes));
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
fn provider_module_faults(file: &ParsedFile<'_>, classes: &ProviderClasses) -> Vec<Fault> {
    let module: String = file_module(file.path);
    let mut faults: Vec<Fault> = Vec::new();
    let mut providers: usize = 0;
    let mut undecided: usize = 0;
    for statement in &file.module.statements {
        let (detail, position): (String, UnitPosition) = match &statement.kind {
            UnitStatementKind::Docstring | UnitStatementKind::Import => continue,
            UnitStatementKind::Assignment(names) if names.is_empty() => {
                (OTHER_STATEMENT.to_owned(), statement.position)
            }
            UnitStatementKind::Assignment(names) => {
                match names.iter().find(|name| !name.starts_with(PRIVATE_PREFIX)) {
                    Some(name) => (
                        format!("public constant '{name}' belongs in constants.py"),
                        statement.position,
                    ),
                    None => continue,
                }
            }
            UnitStatementKind::Class(class) => {
                let verdict: Verdict = classes.verdict(&module, &class.name);
                let detail: Option<String> = class_detail(class, verdict, providers);
                if !class.name.starts_with(PRIVATE_PREFIX) {
                    providers += usize::from(verdict == Verdict::Provider);
                    undecided += usize::from(verdict == Verdict::Unknown);
                }
                match detail {
                    Some(detail) => (detail, class.position),
                    None => continue,
                }
            }
            UnitStatementKind::Function(function) => (
                format!("function '{}' belongs in _helpers/", function.name),
                function.position,
            ),
            UnitStatementKind::Other => (OTHER_STATEMENT.to_owned(), statement.position),
        };
        faults.extend(unit_fault(
            PROVIDER_MODULE_SHAPE_CODE,
            file.path,
            position,
            &detail,
        ));
    }
    if providers == 0 && undecided == 0 {
        faults.extend(unit_fault(
            PROVIDER_MODULE_SHAPE_CODE,
            file.path,
            FILE_START,
            &format!("{} declares no public {PROVIDER_BASE} subclass", file.path),
        ));
    }
    faults
}

/// Why `class` does not belong in provider.py, after `earlier` public Provider subclasses.
fn class_detail(class: &UnitClass, verdict: Verdict, earlier: usize) -> Option<String> {
    let name: &str = &class.name;
    if name.starts_with(PRIVATE_PREFIX) {
        return Some(format!("collaborator class '{name}' belongs in _classes/"));
    }
    match verdict {
        Verdict::NotProvider => Some(format!("class '{name}' does not subclass {PROVIDER_BASE}")),
        Verdict::Provider => {
            (earlier > 0).then(|| format!("'{name}' is a second {PROVIDER_BASE} subclass"))
        }
        Verdict::Unknown => None,
    }
}

/// Whether a class subclasses Provider, as far as the parsed project sources show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Provider,
    NotProvider,
    /// A base leads outside what was parsed, such as another package, so the rule stays silent.
    Unknown,
}

/// What a class's base names: Provider, a project class, a builtin, or something unresolved.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BaseTarget {
    Provider,
    Class(String, String),
    Builtin,
    Unknown,
}

/// The bases of every class in the parsed files, by module and class name.
struct ProviderClasses {
    bases: BTreeMap<(String, String), Vec<BaseTarget>>,
}

impl ProviderClasses {
    /// Whether class `name` of `module` reaches Provider; `Unknown` when a base chain leaves the parsed classes.
    fn verdict(&self, module: &str, name: &str) -> Verdict {
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        let mut pending: Vec<(String, String)> = vec![(module.to_owned(), name.to_owned())];
        let mut unknown: bool = false;
        while let Some(key) = pending.pop() {
            if !seen.insert(key.clone()) {
                continue;
            }
            let Some(bases) = self.bases.get(&key) else {
                unknown = true;
                continue;
            };
            for base in bases {
                match base {
                    BaseTarget::Provider => return Verdict::Provider,
                    BaseTarget::Class(module, name) => pending.push((module.clone(), name.clone())),
                    BaseTarget::Builtin => {}
                    BaseTarget::Unknown => unknown = true,
                }
            }
        }
        if unknown {
            Verdict::Unknown
        } else {
            Verdict::NotProvider
        }
    }
}

/// Every class of `files` with what each of its bases names.
fn provider_classes(files: &[ParsedFile<'_>], modules: &BTreeSet<String>) -> ProviderClasses {
    let mut bases: BTreeMap<(String, String), Vec<BaseTarget>> = BTreeMap::new();
    for file in files {
        let module: String = file_module(file.path);
        for statement in &file.module.statements {
            if let UnitStatementKind::Class(class) = &statement.kind {
                let targets: Vec<BaseTarget> = class
                    .bases
                    .iter()
                    .map(|base| base_target(file, base, modules))
                    .collect();
                bases.insert((module.clone(), class.name.clone()), targets);
            }
        }
    }
    ProviderClasses { bases }
}

/// What `base` names in `file`, resolved through its imports and its own classes.
fn base_target(file: &ParsedFile<'_>, base: &str, modules: &BTreeSet<String>) -> BaseTarget {
    let (first, attributes): (&str, Option<&str>) = match base.split_once('.') {
        Some((first, attributes)) => (first, Some(attributes)),
        None => (base, None),
    };
    if first.is_empty() {
        return BaseTarget::Unknown;
    }
    let folder: &[&str] = &file.parts[..file.parts.len() - 1];
    if let Some(import) = file
        .module
        .imports
        .iter()
        .find(|import| import.local == first)
    {
        let Some(source) = absolute_module(import, folder, modules) else {
            return BaseTarget::Unknown;
        };
        let (module, name): (String, &str) = match (&import.name, attributes) {
            (Some(name), None) if source.ends_with(&format!(".{name}")) => {
                return BaseTarget::Unknown;
            }
            (Some(name), None) => (source, name.as_str()),
            (bound, Some(attributes)) => {
                let bound_module: String = match bound {
                    Some(name) if source.ends_with(&format!(".{name}")) => source,
                    Some(_) => return BaseTarget::Unknown,
                    None if import.module.split('.').next() == Some(import.local.as_str()) => {
                        import.local.clone()
                    }
                    None => source,
                };
                match attributes.rsplit_once('.') {
                    Some((path, name)) => (format!("{bound_module}.{path}"), name),
                    None => (bound_module, attributes),
                }
            }
            (None, None) => return BaseTarget::Unknown,
        };
        return if module.split('.').next() == Some(SQLBUILD_PACKAGE) && name == PROVIDER_BASE {
            BaseTarget::Provider
        } else {
            BaseTarget::Class(module, name.to_owned())
        };
    }
    if attributes.is_some()
        || file
            .module
            .imports
            .iter()
            .any(|import| import.local == STAR_IMPORT)
    {
        return BaseTarget::Unknown;
    }
    let local_class: bool = file.module.statements.iter().any(|statement| {
        matches!(&statement.kind, UnitStatementKind::Class(class) if class.name == base)
    });
    if local_class {
        BaseTarget::Class(file_module(file.path), base.to_owned())
    } else if base == PROVIDER_BASE {
        BaseTarget::Provider
    } else {
        BaseTarget::Builtin
    }
}

/// The dotted module of a project file, such as `providers.storefront_api.provider`.
fn file_module(path: &str) -> String {
    dotted_names(path).pop().unwrap_or_default()
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
