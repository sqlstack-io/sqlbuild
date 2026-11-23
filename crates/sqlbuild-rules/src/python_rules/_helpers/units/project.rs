//! The project's node units, read from parsed `python/` modules, and the unit rules run over them.

use std::collections::BTreeSet;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use sqlbuild_pysource::node_units::main::node_unit_module::node_unit_module;
use sqlbuild_pysource::node_units::models::{
    UnitDecorator, UnitFunction, UnitImport, UnitPosition, UnitStatementKind,
};

use crate::models::{Fault, PythonSource};
use crate::python_rules::_helpers::engine::faults::catalogue_fault;
use crate::python_rules::_helpers::units::imports::import_boundary_faults;
use crate::python_rules::_helpers::units::layout::{
    declared_outside_unit_faults, unit_layout_faults,
};
use crate::python_rules::_helpers::units::modules::module_shape_faults;
use crate::python_rules::_helpers::units::nodes::declaration_faults;
use crate::python_rules::constants::{
    CHECK_KIND, FACTORY_DECORATORS, INIT_FILE, MIN_UNIT_DEPTH, NODE_KIND_DIRECTORIES, NODE_KINDS,
    NODE_MODULES, PACKAGE_MODULE, PACKAGE_ROOT, ROLE_DIRECTORIES, SHARED_UTILITIES_PACKAGE,
    UNIT_CHECKS_FILE, UNIT_DECLARATION_FILES, UNIT_RULE_CODES,
};
use crate::python_rules::models::{ParsedFile, UnitFile, UnitProject};

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

/// The unit folder `parts` declares, when it is a declaration file deep enough and outside every role folder.
fn unit_folder(parts: &[&str]) -> Option<String> {
    let (last, folders) = parts.split_last()?;
    let declares: bool = (UNIT_DECLARATION_FILES.contains(last) || *last == UNIT_CHECKS_FILE)
        && folders.len() >= MIN_UNIT_DEPTH
        && !folders.iter().copied().any(is_role_folder);
    declares.then(|| folders.join("/"))
}

fn unit_project(sources: &[PythonSource], python_version: (u8, u8)) -> UnitProject<'_> {
    let package: Vec<(&PythonSource, Vec<&str>)> = sources
        .iter()
        .filter_map(|source| {
            let rest: &str = package_rest(&source.relative_path)?;
            Some((source, rest.split('/').collect()))
        })
        .collect();
    let units: BTreeSet<String> = unit_folders(sources);
    let files: Vec<UnitFile<'_>> = package
        .par_iter()
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
        index: files
            .iter()
            .enumerate()
            .map(|(position, file)| (file.path, position))
            .collect(),
        node_modules: units
            .iter()
            .flat_map(|unit| {
                NODE_MODULES.iter().map(move |module| {
                    format!("{PACKAGE_MODULE}.{}.{module}", unit.replace('/', "."))
                })
            })
            .collect(),
        files,
        units,
        paths: sources
            .iter()
            .map(|source| source.relative_path.as_str())
            .collect(),
        modules: project_modules(sources),
    }
}

/// Every dotted module the project's Python files define, packages included.
pub(crate) fn project_modules(sources: &[PythonSource]) -> BTreeSet<String> {
    sources
        .iter()
        .flat_map(|source| dotted_names(&source.relative_path))
        .collect()
}

/// The files under the folder `root/` or whose text names its module, parsed; a file that does not parse is left out.
pub(crate) fn files_naming<'a>(
    sources: &'a [PythonSource],
    (root, module): (&str, &str),
    python_version: (u8, u8),
) -> Vec<ParsedFile<'a>> {
    sources
        .par_iter()
        .filter(|source| {
            source
                .relative_path
                .strip_prefix(root)
                .is_some_and(|rest| rest.starts_with('/'))
                || source.text.contains(module)
        })
        .filter_map(|source| {
            Some(ParsedFile {
                path: source.relative_path.as_str(),
                parts: source.relative_path.split('/').collect(),
                text: source.text.as_str(),
                module: node_unit_module(&source.text, python_version)?,
            })
        })
        .collect()
}

/// Each import in `file` of a project module that `reason` rejects, as a fault of its rule and detail.
pub(crate) fn rejected_import_faults(
    file: &ParsedFile<'_>,
    modules: &BTreeSet<String>,
    reason: impl Fn(&str) -> Option<(&'static str, String)>,
) -> Vec<Fault> {
    let folder: &[&str] = &file.parts[..file.parts.len() - 1];
    file.module
        .imports
        .iter()
        .filter_map(|import| {
            let module: String = absolute_module(import, folder, modules)?;
            if !modules.contains(&module) {
                return None;
            }
            let (code, detail): (&str, String) = reason(&module)?;
            unit_fault(
                code,
                file.path,
                import.position,
                &format!("{} imports {module}, {detail}", file.path),
            )
        })
        .collect()
}

/// The path of a `python/nodes/` file below that folder.
pub(crate) fn package_rest(path: &str) -> Option<&str> {
    path.strip_prefix(PACKAGE_ROOT)?.strip_prefix('/')
}

/// The dotted parts of `module` below `package`, when it sits in that package.
pub(crate) fn module_below<'m>(module: &'m str, package: &str) -> Option<Vec<&'m str>> {
    let rest: &str = module.strip_prefix(package)?;
    if rest.is_empty() {
        return Some(Vec::new());
    }
    Some(rest.strip_prefix('.')?.split('.').collect())
}

/// The unit folders under `python/nodes/`, each relative to it.
pub(crate) fn unit_folders(sources: &[PythonSource]) -> BTreeSet<String> {
    sources
        .iter()
        .filter_map(|source| {
            let parts: Vec<&str> = package_rest(&source.relative_path)?.split('/').collect();
            unit_folder(&parts)
        })
        .collect()
}

/// The deepest unit folder among the file's folders.
pub(crate) fn nearest_unit(parts: &[&str], units: &BTreeSet<String>) -> Option<String> {
    (1..parts.len())
        .rev()
        .map(|depth| parts[..depth].join("/"))
        .find(|folder| units.contains(folder))
}

/// The dotted module of a Python path and of every package above it.
pub(crate) fn dotted_names(path: &str) -> Vec<String> {
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
    project
        .index
        .get(path.as_str())
        .and_then(|position| project.files.get(*position))
}

/// Whether a folder under `python/` holds shared or role code rather than a domain or a unit.
pub(crate) fn is_role_folder(folder: &str) -> bool {
    folder == SHARED_UTILITIES_PACKAGE
        || NODE_KIND_DIRECTORIES.contains(&folder)
        || ROLE_DIRECTORIES.iter().any(|(name, _)| *name == folder)
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
    function.decorators.iter().find_map(declaration)
}

/// What `decorator` declares, with the decorator, if anything.
fn declaration(decorator: &UnitDecorator) -> Option<(Declared, &UnitDecorator)> {
    Some((decorator_declares(&decorator.name)?, decorator))
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
    let mut folder: Vec<&str> = PACKAGE_MODULE.split('.').collect();
    folder.extend(&file.parts[..file.parts.len() - 1]);
    absolute_module(import, &folder, &project.modules)
}

/// The absolute dotted module `import` reads from a file in the project folder `folder`, given every project module.
pub(crate) fn absolute_module(
    import: &UnitImport,
    folder: &[&str],
    modules: &BTreeSet<String>,
) -> Option<String> {
    let mut base: Vec<&str> = Vec::new();
    if import.level > 0 {
        base.extend(folder);
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
    Some(if modules.contains(&submodule) {
        submodule
    } else {
        module
    })
}

/// The imports in `file` of `python.<unit>.<module>`, one of `modules`, or of names from it.
pub(crate) fn unit_imports<'f>(
    file: &'f UnitFile<'_>,
    project: &UnitProject<'_>,
    unit: &str,
    modules: &[&str],
) -> Vec<&'f UnitImport> {
    let targets: Vec<String> = modules
        .iter()
        .map(|module| format!("{PACKAGE_MODULE}.{}.{module}", unit.replace('/', ".")))
        .collect();
    file.module
        .imports
        .iter()
        .filter(|import| import.name.is_some())
        .filter(|import| {
            imported_module(import, file, project).is_some_and(|module| targets.contains(&module))
        })
        .collect()
}

/// The names `file` binds by importing from `python.<unit>.<module>`, one of `modules`.
pub(crate) fn names_from_unit_modules(
    file: &UnitFile<'_>,
    project: &UnitProject<'_>,
    unit: &str,
    modules: &[&str],
) -> BTreeSet<String> {
    unit_imports(file, project, unit, modules)
        .into_iter()
        .map(|import| import.local.clone())
        .collect()
}

/// Whether `import` binds a module, as `from python.orders.daily import main` binds `main.py` itself.
pub(crate) fn binds_module(
    import: &UnitImport,
    file: &UnitFile<'_>,
    project: &UnitProject<'_>,
) -> bool {
    let Some(name) = &import.name else {
        return true;
    };
    let package = UnitImport {
        name: None,
        ..import.clone()
    };
    imported_module(import, file, project)
        == imported_module(&package, file, project).map(|module| format!("{module}.{name}"))
}
