//! The command's target by the same selected, local, then project default precedence as compilation.

use std::path::Path;

use sqlbuild_config::effective::main::effective_target_name::effective_target_name;
use sqlbuild_config::effective::models::TargetChoice;
use sqlbuild_config::models::ConfigValue;
use sqlbuild_config::project::main::load_local_config::load_local_config;
use sqlbuild_config::project::main::load_project_config::load_project_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_config::project::models::Built;

use crate::native_commands::_helpers::project_facts::object_field;

/// The effective target of `project_dir` with `selected` as `--target`, `None` when nothing selects one.
pub fn effective_target(
    project_dir: &Path,
    selected: Option<String>,
) -> Result<Option<String>, String> {
    let paths = project_config_paths(project_dir);
    let invalid = |_| "project configuration is invalid; run sqb debug".to_owned();
    let project = load_project_config(&paths).map_err(invalid)?;
    let local = load_local_config(&paths).map_err(invalid)?;
    let text = |config: &Built, name: &str| match object_field(config, name) {
        Some(Built::Value(ConfigValue::String(value))) => Some(value.clone()),
        _ => None,
    };
    let defined: Vec<String> = [&project, &local]
        .into_iter()
        .filter_map(|config| object_field(config, "targets"))
        .filter_map(|targets| match targets {
            Built::Dict(targets) => Some(targets),
            _ => None,
        })
        .flatten()
        .map(|(name, _)| name.clone())
        .collect();
    effective_target_name(TargetChoice {
        selected,
        local: text(&local, "target"),
        default: text(&project, "default_target"),
        defined,
    })
    .map_err(|_| "unknown target; run sqb debug --target <name>".to_owned())
}
