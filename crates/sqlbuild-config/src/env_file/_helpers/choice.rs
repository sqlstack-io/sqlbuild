//! The `env_file` the project and the selected target set.

use std::path::Path;

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

use crate::env_file::constants::DEFAULT_ENV_FILE;
use crate::env_file::models::{ChoiceSource, EnvFileChoice};
use crate::models::ConfigValue;
use crate::project::main::load_project_config::load_project_config;
use crate::project::main::project_config_paths::project_config_paths;
use crate::project::models::Built;

fn field<'a>(built: &'a Built, name: &str) -> Option<&'a Built> {
    match built {
        Built::Object(_, fields) => fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value),
        Built::Dict(entries) => entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value),
        _ => None,
    }
}

fn text(built: Option<&Built>) -> Option<String> {
    match built {
        Some(Built::Value(ConfigValue::String(text))) => Some(text.clone()),
        _ => None,
    }
}

/// The selected target's `env_file`, else the project's, with where it was set.
pub(crate) fn configured_source(
    project: &Built,
    (project_dir, target): (&Path, Option<&str>),
) -> ChoiceSource {
    let from_target: Option<String> = target.and_then(|name| {
        text(
            field(project, "targets")
                .and_then(|targets| field(targets, name))
                .and_then(|config| field(config, "env_file")),
        )
    });
    if from_target.is_some() {
        return ChoiceSource {
            text: from_target,
            target: target.map(str::to_owned),
            project_dir: project_dir.to_path_buf(),
        };
    }
    ChoiceSource {
        text: text(field(project, "env_file")),
        target: None,
        project_dir: project_dir.to_path_buf(),
    }
}

/// The path to read, relative to the project directory unless absolute; `""` turns loading off.
pub(crate) fn choice(source: ChoiceSource) -> EnvFileChoice {
    let configured: bool = source.text.is_some();
    let written: String = source.text.unwrap_or_else(|| DEFAULT_ENV_FILE.to_owned());
    let setting: String = match (&source.target, configured) {
        (Some(target), _) => format!("[targets.{target}] env_file in {PROJECT_CONFIG_FILENAME}"),
        (None, true) => format!("env_file in {PROJECT_CONFIG_FILENAME}"),
        (None, false) => format!("the default env_file = \"{DEFAULT_ENV_FILE}\""),
    };
    EnvFileChoice {
        path: (!written.is_empty()).then(|| source.project_dir.join(&written)),
        display: written,
        configured,
        setting,
    }
}

/// The chosen env file, `None` without a valid `sqlbuild_project.toml`, which the command itself reports.
pub(crate) fn env_file_choice(project_dir: &Path, target: Option<&str>) -> Option<EnvFileChoice> {
    let paths = project_config_paths(project_dir);
    if !paths.project.is_file() {
        return None;
    }
    match load_project_config(&paths) {
        Ok(project) => Some(choice(configured_source(&project, (project_dir, target)))),
        Err(_invalid) => None,
    }
}
