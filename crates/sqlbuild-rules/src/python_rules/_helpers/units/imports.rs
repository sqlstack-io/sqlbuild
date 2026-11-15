//! Which project modules a unit's files may import.

use sqlbuild_pysource::node_units::models::UnitImport;

use crate::models::Fault;
use crate::python_rules::_helpers::units::project::{imported_module, unit_fault};
use crate::python_rules::constants::{
    NODE_IMPORT_BOUNDARIES_CODE, PACKAGE_ROOT, PROVIDER_ROLE_MODULES, PROVIDERS_ROOT,
    SHARED_ROLE_MODULES, SHARED_UTILITIES_PACKAGE, UNIT_HELPERS_FOLDER,
};
use crate::python_rules::models::UnitProject;

/// Each import in a unit's files of a project module outside the unit's boundaries.
pub(crate) fn import_boundary_faults(project: &UnitProject<'_>) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    for file in &project.files {
        let Some(unit) = file.unit.as_deref() else {
            continue;
        };
        for import in &file.module.imports {
            let Some(module) = imported_module(import, file, project) else {
                continue;
            };
            if !project.modules.contains(&module) || allowed(&module, import, unit, project) {
                continue;
            }
            faults.extend(unit_fault(
                NODE_IMPORT_BOUNDARIES_CODE,
                file.path,
                import.position,
                &format!(
                    "{} imports {module}, which is outside its unit's boundaries",
                    file.path
                ),
            ));
        }
    }
    faults
}

/// Whether a file of `unit` may import the project module `module` through `import`.
fn allowed(module: &str, import: &UnitImport, unit: &str, project: &UnitProject<'_>) -> bool {
    let unit_parts: Vec<&str> = unit.split('/').collect();
    let own: String = format!("{PACKAGE_ROOT}.{}", unit_parts.join("."));
    let mut shared: Vec<String> = vec![own, format!("{PACKAGE_ROOT}.{SHARED_UTILITIES_PACKAGE}")];
    for depth in 1..unit_parts.len() {
        let ancestor: String = format!("{PACKAGE_ROOT}.{}", unit_parts[..depth].join("."));
        shared.extend(
            SHARED_ROLE_MODULES
                .iter()
                .chain([&UNIT_HELPERS_FOLDER])
                .map(|role| format!("{ancestor}.{role}")),
        );
    }
    if shared.iter().any(|prefix| within(module, prefix)) || provider_role(module) {
        return true;
    }
    project.node_modules.contains(module) && import.only_in_depends_on
}

/// Whether `module` is a public role file of a provider capability, `providers.<capability>.<role>`.
fn provider_role(module: &str) -> bool {
    let parts: Vec<&str> = module.split('.').collect();
    matches!(
        parts.as_slice(),
        [root, _, role, ..] if *root == PROVIDERS_ROOT && PROVIDER_ROLE_MODULES.contains(role)
    )
}

/// Whether `module` is `package` or sits below it.
fn within(module: &str, package: &str) -> bool {
    module
        .strip_prefix(package)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}
