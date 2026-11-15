//! What a module-level name selects as a sensor condition, read from the module that binds it.

use ruff_text_size::TextSize;

use crate::cpython_tree::main::parse_cpython_module::parse_cpython_module;
use crate::cpython_tree::models::ParsedSource;
use crate::orchestration_modules::_helpers::calls::imported_names;
use crate::orchestration_modules::_helpers::conditions::condition_source;
use crate::orchestration_modules::_helpers::declarations::module_scope;
use crate::orchestration_modules::models::{ConditionSource, ModuleScope, SourcePosition};

/// What `name`, as `contents` leaves it bound, selects; `None` for a condition that selects no nodes.
#[must_use]
pub fn module_binding_condition(
    contents: &str,
    python_version: (u8, u8),
    name: &str,
) -> Option<ConditionSource> {
    let start = SourcePosition { line: 1, column: 1 };
    let Some(parsed): Option<ParsedSource> = parse_cpython_module(contents, python_version) else {
        return Some(ConditionSource::Unresolvable {
            what: format!(
                "the module binding `{name}` is not valid Python {}.{}",
                python_version.0, python_version.1
            ),
            position: start,
        });
    };
    let imports: Vec<(String, String, TextSize)> = imported_names(&parsed.module.body);
    let scope: ModuleScope<'_> = module_scope(&parsed, contents, &imports);
    let assigned = scope
        .assignments
        .get(name)
        .and_then(|bindings| bindings.last());
    let imported = scope.imports.get(name).and_then(|bindings| bindings.last());
    match (assigned, imported) {
        (Some((at, value)), imported) if imported.is_none_or(|import| import.start < *at) => {
            condition_source(value, *at, &scope)
        }
        (_, Some(import)) => Some(ConditionSource::Imported {
            module: import.module.clone(),
            level: import.level,
            name: import.name.to_owned(),
            position: start,
        }),
        _ => Some(ConditionSource::Unresolvable {
            what: format!("`{name}` is not bound at module level"),
            position: start,
        }),
    }
}
