//! Project facts that decide, before any side effect, whether a command needs Python.

use std::path::Path;

use sqlbuild_config::models::ConfigValue;
use sqlbuild_config::project::errors::{LoadFailure, MessagePiece};
use sqlbuild_config::project::main::load_local_config::load_local_config;
use sqlbuild_config::project::main::load_project_config::load_project_config;
use sqlbuild_config::project::main::project_config_paths::project_config_paths;
use sqlbuild_config::project::models::{Built, ConfigFile, ConfigPaths};
use sqlbuild_core::screen_text::models::{Problem, ProblemLevel};
use sqlbuild_orchestration::store::errors::ControlError;

use crate::native_commands::main::effective_target::effective_target;

use crate::native_commands::constants::{
    CONFIG_ERROR_CODE, CUSTOM_RULES_DIRECTORY, EVENT_SINKS_DIRECTORY, LIFECYCLE_SINK_PATH,
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
    if project_dir.join(EVENT_SINKS_DIRECTORY).is_dir() {
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

/// The dev target and configured adapter, or the configuration's problem as the screen shows it.
pub(crate) fn development_config(
    project_dir: &Path,
    selected: Option<String>,
) -> Result<(String, String), Box<Problem>> {
    let paths: ConfigPaths = project_config_paths(project_dir);
    let project: Built =
        load_project_config(&paths).map_err(|failure| load_problem(&paths, failure))?;
    load_local_config(&paths).map_err(|failure| load_problem(&paths, failure))?;
    let target_problem = |why: String| {
        Box::new(Problem {
            level: ProblemLevel::Error,
            title: "target".to_owned(),
            code: String::new(),
            at: Vec::new(),
            why,
            fix: "pass --target <name>, or set default_target in sqlbuild_project.toml".to_owned(),
        })
    };
    let target: String = effective_target(project_dir, selected)
        .map_err(target_problem)?
        .ok_or_else(|| target_problem("sqb dev needs a target and none is selected".to_owned()))?;
    let adapter: String = match object_field(&project, "adapter") {
        Some(Built::Value(ConfigValue::String(adapter))) => adapter.clone(),
        _ => String::new(),
    };
    Ok((target, adapter))
}

/// A configuration file the loader rejected, with its message and help in full.
fn load_problem(paths: &ConfigPaths, failure: LoadFailure) -> Box<Problem> {
    let name = |file: ConfigFile| -> String {
        let path = match file {
            ConfigFile::Project => &paths.project,
            ConfigFile::LegacyProject => &paths.legacy_project,
            ConfigFile::Local => &paths.local,
            ConfigFile::LegacyLocal => &paths.legacy_local,
        };
        path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    };
    let problem = |title: String, why: String| Problem {
        level: ProblemLevel::Error,
        title,
        code: String::new(),
        at: Vec::new(),
        why,
        fix: "sqb debug checks the configuration on its own".to_owned(),
    };
    Box::new(match failure {
        LoadFailure::Config(failure) => {
            let title: String = failure
                .message
                .iter()
                .find_map(|piece| match piece {
                    MessagePiece::File(file) => Some(name(*file)),
                    MessagePiece::Text(_) => None,
                })
                .unwrap_or_else(|| "configuration".to_owned());
            let why: String = failure.render(|_| String::new()).trim().to_owned();
            Problem {
                code: CONFIG_ERROR_CODE.to_owned(),
                fix: failure
                    .help
                    .unwrap_or_else(|| "sqb debug checks the configuration on its own".to_owned()),
                ..problem(title, why)
            }
        }
        LoadFailure::Io(file, error) => problem(name(file), format!("could not read it: {error}")),
        LoadFailure::Utf8(_) => problem(
            "configuration".to_owned(),
            "a configuration file is not UTF-8 text".to_owned(),
        ),
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
