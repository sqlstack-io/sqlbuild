//! Every project module at once, for the rules that judge how modules use each other.

use std::path::Path;

use fensu_native::rules::constants::{DEAD_DEFINITION_CODE, PUBLIC_MAIN_ENTRY_EXTERNAL_USE_CODE};
use fensu_native::{NativeProjectModule, NativeProjectPlane};
use sqlbuild_pysource::definition_locations::main::decorated_functions::decorated_functions;

use crate::models::EvaluateRequest;
use crate::python_rules::_helpers::programs::Programs;
use crate::python_rules::constants::{
    ENTRYPOINT_KIND, INIT_FILE, NODE_DECORATORS, PACKAGE_ROOT, ROOT_SCOPE,
};
use crate::python_rules::models::{PythonRuleSpec, PythonTarget};

/// The project plane and the definitions SQLBuild loads, built only for the rules that read them.
pub(crate) fn loaded_project(
    rules: &[&PythonRuleSpec],
    targets: &[PythonTarget],
    programs: &Programs<'_>,
    request: &EvaluateRequest,
) -> Result<(NativeProjectPlane, Vec<(String, String)>), String> {
    let reads_plane: bool = rules.iter().any(|rule| {
        matches!(
            rule.fensu_code,
            Some(PUBLIC_MAIN_ENTRY_EXTERNAL_USE_CODE | DEAD_DEFINITION_CODE)
        )
    });
    if !reads_plane {
        return Ok((NativeProjectPlane::default(), Vec::new()));
    }
    let ownership_root: String = Path::new(&request.project_dir)
        .join(PACKAGE_ROOT)
        .to_string_lossy()
        .into_owned();
    project_plane(targets, programs, &ownership_root, request.python_version)
}

/// Every module, with the modules and definitions SQLBuild loads: all outside `python/`, node functions inside.
fn project_plane(
    targets: &[PythonTarget],
    programs: &Programs<'_>,
    ownership_root: &str,
    python_version: (u8, u8),
) -> Result<(NativeProjectPlane, Vec<(String, String)>), String> {
    let mut modules: Vec<NativeProjectModule> = Vec::with_capacity(targets.len());
    let mut entry_modules: Vec<String> = Vec::new();
    let mut entrypoints: Vec<(String, String)> = Vec::new();
    for target in targets {
        let module_parts: Vec<String> = module_parts(&target.repository_path);
        let module_name: String = module_parts.join(".");
        let program = programs.program(target.source)?;
        let loaded: Vec<String> = if target.package {
            decorated_functions(
                &programs.sources[target.source].text,
                &NODE_DECORATORS,
                python_version,
            )
        } else {
            program
                .declaration_rows()
                .statements
                .iter()
                .filter_map(|row| row.function_name.clone().or_else(|| row.class_name.clone()))
                .collect()
        };
        if !target.package || !loaded.is_empty() {
            entry_modules.push(module_name.clone());
        }
        entrypoints.extend(
            loaded
                .into_iter()
                .map(|name| (ENTRYPOINT_KIND.to_owned(), format!("{module_name}:{name}"))),
        );
        let module = NativeProjectModule::new(
            target.repository_path.clone(),
            ROOT_SCOPE.to_owned(),
            module_parts,
            program.clone(),
        );
        modules.push(if target.package {
            module.with_ownership(Some(1), Some(ownership_root.to_owned()))
        } else {
            module.with_ownership(None, None)
        });
    }
    Ok((NativeProjectPlane::new(modules, entry_modules), entrypoints))
}

/// The dotted-name parts of the module at project-relative `path`.
fn module_parts(path: &str) -> Vec<String> {
    let mut parts: Vec<String> = path.split('/').map(str::to_owned).collect();
    if parts.last().is_some_and(|name| name == INIT_FILE) {
        parts.pop();
    } else if let Some(last) = parts.last_mut() {
        *last = last.strip_suffix(".py").unwrap_or(last).to_owned();
    }
    parts
}
