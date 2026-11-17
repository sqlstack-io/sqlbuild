//! Project facts that decide, before any side effect, whether a command needs Python.

use std::path::Path;

use sqlbuild_config::models::ConfigValue;
use sqlbuild_config::project::main::load_project_config::load_project_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_config::project::models::{Built, ConfigPaths};
use sqlbuild_orchestration::store::errors::ControlError;

use crate::native_commands::main::effective_target::effective_target;

use crate::native_commands::constants::{
    CUSTOM_RULES_DIRECTORY, EVENT_EXPORTERS_DIRECTORY, LIFECYCLE_SINK_PATH, SINKS_DIRECTORY,
};

pub(crate) fn object_field<'a>(built: &'a Built, name: &str) -> Option<&'a Built> {
    match built {
        Built::Object(_, fields) => fields
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| value),
        _ => None,
    }
}

/// Whether the loaded configuration names a lifecycle sink, which only Python can bind.
fn names_lifecycle_sinks(config: &Built) -> bool {
    let named: Option<&Built> = LIFECYCLE_SINK_PATH
        .iter()
        .try_fold(config, |value, name| object_field(value, name));
    match named {
        None => false,
        Some(Built::Dict(entries)) => !entries.is_empty(),
        Some(_) => true,
    }
}

/// Whether `discover_runtime_extensions` would import nothing and accept the project file.
pub(crate) fn has_no_runtime_extensions(project_dir: &Path) -> bool {
    if project_dir.join(EVENT_EXPORTERS_DIRECTORY).exists()
        || project_dir.join(SINKS_DIRECTORY).is_dir()
    {
        return false;
    }
    let paths: ConfigPaths = project_config_paths(project_dir);
    if !paths.project.exists() {
        return true;
    }
    match load_project_config(&paths) {
        Ok(config) => !names_lifecycle_sinks(&config),
        Err(_) => false,
    }
}

/// Whether the project defines custom rules, which Python imports.
pub(crate) fn has_custom_rules(project_dir: &Path) -> bool {
    project_dir.join(CUSTOM_RULES_DIRECTORY).is_dir()
}

/// The project's required `name`, which names its development location.
pub(crate) fn development_project_name(project_dir: &Path) -> Result<String, ControlError> {
    let invalid =
        || ControlError::Invalid("project configuration is invalid; run sqb debug".into());
    let config = load_project_config(&project_config_paths(project_dir)).map_err(|_| invalid())?;
    match object_field(&config, "name") {
        Some(Built::Value(ConfigValue::String(name))) if !name.trim().is_empty() => {
            Ok(name.clone())
        }
        _ => Err(invalid()),
    }
}

/// Resolve development's target through the same selected/local/default precedence as compilation.
pub(crate) fn development_target(
    project_dir: &Path,
    selected: Option<String>,
) -> Result<String, ControlError> {
    effective_target(project_dir, selected)
        .map_err(ControlError::Invalid)?
        .ok_or_else(|| {
            ControlError::Invalid("sqb dev requires --target or a configured default target".into())
        })
}

/// The project's configured name, or its directory's name when the configuration has none.
pub(crate) fn project_name(project_dir: &Path) -> String {
    let paths = project_config_paths(project_dir);
    let configured: Option<String> = match load_project_config(&paths) {
        Ok(config) => match object_field(&config, "name") {
            Some(Built::Value(ConfigValue::String(name))) => Some(name.clone()),
            _ => None,
        },
        Err(_) => None,
    };
    configured.unwrap_or_else(|| project_directory_name(project_dir))
}

/// The last component of the project directory.
pub(crate) fn project_directory_name(project_dir: &Path) -> String {
    project_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
