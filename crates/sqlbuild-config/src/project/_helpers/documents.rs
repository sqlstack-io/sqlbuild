//! Read the configuration documents and assemble the project and local configurations.

use crate::errors::ConfigError;
use crate::models::ConfigValue;
use crate::project::_helpers::defaults::{
    load_defaults, load_materialization_defaults, load_path_defaults,
};
use crate::project::_helpers::fields::{
    coerce_mapping, get, has, object, optional_mapping, optional_str, require_str, string_mapping,
    text_or_none,
};
use crate::project::_helpers::help::{
    Setting, SettingValue, join_helps, setting_help, setting_note,
};
use crate::project::_helpers::keys::Place;
use crate::project::_helpers::sections::{
    load_constants, load_control, load_cost, load_cursors, load_dbt, load_diff, load_git,
    load_janitor, load_local_dbt, load_microbatches, load_migrations, load_policy_flag,
    load_review, load_scenario, load_settings, load_snapshots, setting_overrides,
};
use crate::project::_helpers::sinks::load_sinks;
use crate::project::_helpers::targets::{
    load_connections, load_target_fields, local_target_config, project_target,
};
use crate::project::_helpers::values::is_truthy;
use crate::project::constants::{
    COLUMN_CONTRACT_MODES, LEGACY_COLUMN_CONTRACT_MODE_KEY, LEGACY_EVENT_EXPORTERS_KEY,
    LOCAL_CONFIG_FILENAME, QUERY_CHANGE_TRACKING_KEY, REQUIRE_SQL_ANALYSIS_KEY, SQL_ANALYSIS_KEY,
};
use crate::project::errors::{ConfigFailure, FailureClass, LoadFailure, MessagePiece};
use crate::project::models::{Built, ConfigFile, ConfigPaths, ControlConfig, ReviewConfig};
use crate::project::types::{Entries, Fields, Loaded};
use crate::recall::constants::RECALL_SECTION;
use crate::recall::main::recall_section::recall_section;
use crate::toml::main::load_toml::load_toml;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::constants::SETTINGS_SECTION;
use std::path::Path;

const PROJECT_FILE: ConfigFile = ConfigFile::Project;
const LOCAL_FILE: ConfigFile = ConfigFile::Local;

/// The path of one configuration file.
pub(crate) fn path_of(paths: &ConfigPaths, file: ConfigFile) -> &Path {
    match file {
        ConfigFile::Project => &paths.project,
        ConfigFile::LegacyProject => &paths.legacy_project,
        ConfigFile::Local => &paths.local,
        ConfigFile::LegacyLocal => &paths.legacy_local,
    }
}

/// The TOML file to read, rejecting a legacy YAML file that has no TOML replacement.
pub(crate) fn resolve(paths: &ConfigPaths, local: bool) -> Result<ConfigFile, ConfigFailure> {
    let (file, legacy, name) = if local {
        (
            ConfigFile::Local,
            ConfigFile::LegacyLocal,
            LOCAL_CONFIG_FILENAME,
        )
    } else {
        (
            ConfigFile::Project,
            ConfigFile::LegacyProject,
            PROJECT_CONFIG_FILENAME,
        )
    };
    if path_of(paths, file).exists() || !path_of(paths, legacy).exists() {
        return Ok(file);
    }
    Err(ConfigFailure::at(
        legacy,
        format!("is no longer supported. Rename it to {name} and convert the contents to TOML."),
    ))
}

fn invalid_toml(file: ConfigFile, error: &ConfigError) -> ConfigFailure {
    let position = match (error.line, error.column) {
        (Some(line), Some(column)) => format!(" (at line {line}, column {column})"),
        (Some(line), None) => format!(" (at line {line})"),
        _ => String::new(),
    };
    ConfigFailure::at(
        file,
        format!("contains invalid TOML: {}{position}", error.message),
    )
}

/// `_load_toml_mapping`: the top-level table of a TOML file.
pub(crate) fn load_toml_mapping(
    paths: &ConfigPaths,
    file: ConfigFile,
) -> Result<ConfigValue, LoadFailure> {
    let path = path_of(paths, file);
    if !path.exists() {
        return Err(LoadFailure::Config(ConfigFailure {
            message: vec![
                MessagePiece::Text("Project config not found: ".to_owned()),
                MessagePiece::File(file),
                MessagePiece::Text(
                    ". Run sqb from a sqlbuild project directory or pass --project-dir.".to_owned(),
                ),
            ],
            help: None,
            class: FailureClass::ProjectConfig,
        }));
    }
    let bytes = std::fs::read(path).map_err(|error| LoadFailure::Io(file, error))?;
    let text = String::from_utf8(bytes).map_err(|error| LoadFailure::Utf8(error.into_bytes()))?;
    load_toml(&text).map_err(|error| LoadFailure::Config(invalid_toml(file, &error)))
}

fn flag(built: &Built, name: &str) -> Option<bool> {
    let Built::Object(_, fields) = built else {
        return None;
    };
    fields
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| matches!(value, Built::Value(ConfigValue::Bool(true))))
}

fn tracking_setting(value: bool) -> Setting<'static> {
    Setting {
        file_name: PROJECT_CONFIG_FILENAME,
        section: SETTINGS_SECTION,
        key: QUERY_CHANGE_TRACKING_KEY,
        value: SettingValue::Bool(value),
    }
}

fn reject_untracked_janitor(janitor: &Built, settings: &Built) -> Loaded<()> {
    let enabled = flag(janitor, "enabled") == Some(true);
    let tracked_only = flag(janitor, "delete_tracked_only") == Some(true);
    if !enabled || !tracked_only || flag(settings, QUERY_CHANGE_TRACKING_KEY) != Some(false) {
        return Ok(());
    }
    Err(ConfigFailure::at(
        PROJECT_FILE,
        format!(
            "[janitor] delete_tracked_only = true requires query change tracking; {}",
            setting_note(&tracking_setting(false))
        ),
    )
    .with_help(setting_help(
        "to track which relations SQLBuild created",
        &tracking_setting(true),
    )))
}

fn identity(payload: &Entries) -> Loaded<Fields> {
    Ok(vec![
        (
            "name",
            Built::Value(ConfigValue::String(require_str(
                payload,
                "name",
                PROJECT_FILE,
            )?)),
        ),
        (
            "adapter",
            Built::Value(ConfigValue::String(require_str(
                payload,
                "adapter",
                PROJECT_FILE,
            )?)),
        ),
        (
            "default_target",
            text_or_none(optional_str(payload, "default_target")?),
        ),
        (
            "connection",
            Built::Value(optional_mapping(payload, "connection")?),
        ),
        (
            "connections",
            load_connections(get(payload, "connections"), PROJECT_FILE)?,
        ),
    ])
}

fn sections(payload: &Entries) -> Loaded<Fields> {
    Ok(vec![
        (
            "settings",
            load_settings(get(payload, "settings"), PROJECT_FILE)?,
        ),
        (
            "scopes",
            load_policy_flag(
                get(payload, "scopes"),
                ("scopes", "enforce_placement", "ScopesConfig"),
                PROJECT_FILE,
            )?,
        ),
        (
            "references",
            load_policy_flag(
                get(payload, "references"),
                ("references", "enforce_explicit", "ReferencesConfig"),
                PROJECT_FILE,
            )?,
        ),
        ("cost", load_cost(get(payload, "cost"), PROJECT_FILE)?),
        (
            "constants",
            load_constants(get(payload, "constants"), PROJECT_FILE)?,
        ),
        (
            "cursors",
            load_cursors(get(payload, "cursors"), PROJECT_FILE)?,
        ),
        (
            "microbatches",
            load_microbatches(get(payload, "microbatches"), PROJECT_FILE)?,
        ),
        (
            "defaults",
            load_defaults(get(payload, "defaults"), PROJECT_FILE)?,
        ),
        ("diff", load_diff(get(payload, "diff"), PROJECT_FILE)?),
        (
            "materialization_defaults",
            load_materialization_defaults(get(payload, "materialization_defaults"), PROJECT_FILE)?,
        ),
        (
            "path_defaults",
            load_path_defaults(get(payload, "path_defaults"), PROJECT_FILE)?,
        ),
        ("vars", string_mapping(get(payload, "vars"), PROJECT_FILE)?),
    ])
}

fn targets(payload: &Entries) -> Loaded<Built> {
    Ok(Built::Dict(
        load_target_fields(get(payload, "targets"), PROJECT_FILE)?
            .into_iter()
            .map(|(name, fields)| (name, project_target(fields)))
            .collect(),
    ))
}

fn operations(payload: &Entries) -> Loaded<Fields> {
    Ok(vec![
        (
            "janitor",
            load_janitor(get(payload, "janitor"), PROJECT_FILE)?,
        ),
        (
            "migrations",
            load_migrations(get(payload, "migrations"), PROJECT_FILE)?,
        ),
        (
            "snapshots",
            load_snapshots(get(payload, "snapshots"), PROJECT_FILE)?,
        ),
        (
            "scenario",
            load_scenario(get(payload, "scenario"), PROJECT_FILE)?,
        ),
        ("dbt", load_dbt(get(payload, "dbt"), PROJECT_FILE)?),
        ("sinks", load_sinks(get(payload, "sinks"), PROJECT_FILE)?),
    ])
}

fn project_fields(payload: &Entries) -> Loaded<Fields> {
    if get(payload, LEGACY_EVENT_EXPORTERS_KEY).is_some() {
        return Err(ConfigFailure::at(
            PROJECT_FILE,
            "event_exporters was replaced by sinks.lifecycle",
        ));
    }
    if let Some(recall) = get(payload, RECALL_SECTION) {
        recall_section(recall, RECALL_SECTION)
            .map_err(|message| ConfigFailure::at(PROJECT_FILE, message))?;
    }
    load_git(get(payload, "git"), PROJECT_FILE)?;
    load_review(get(payload, "review"), PROJECT_FILE)?;
    load_control(get(payload, "control"), PROJECT_FILE)?;
    let mut fields = identity(payload)?;
    fields.extend(sections(payload)?);
    fields.push(("targets", targets(payload)?));
    let operations = operations(payload)?;
    let settings = &fields[5].1;
    reject_untracked_janitor(&operations[0].1, settings)?;
    fields.extend(operations);
    Ok(fields)
}

/// `load_project_config`: the validated `ProjectConfig`.
pub(crate) fn project_config(paths: &ConfigPaths) -> Result<Built, LoadFailure> {
    let file = resolve(paths, false)?;
    let table = load_toml_mapping(paths, file)?;
    let payload = table.as_map().unwrap_or_default();
    Ok(object("ProjectConfig", project_fields(payload)?))
}

fn shared_policy_failure(value: &ConfigValue) -> ConfigFailure {
    let help = join_helps(&[
        format!("remove {REQUIRE_SQL_ANALYSIS_KEY} from {LOCAL_CONFIG_FILENAME}"),
        setting_help(
            "to change the policy for everyone",
            &Setting {
                file_name: PROJECT_CONFIG_FILENAME,
                section: SETTINGS_SECTION,
                key: REQUIRE_SQL_ANALYSIS_KEY,
                value: SettingValue::Bool(!is_truthy(value)),
            },
        ),
    ]);
    ConfigFailure::at(
        LOCAL_FILE,
        format!(
            "cannot set [settings] {REQUIRE_SQL_ANALYSIS_KEY}; it is a shared project policy \
             that only {PROJECT_CONFIG_FILENAME} can set"
        ),
    )
    .with_help(help)
}

fn local_settings(payload: Option<&ConfigValue>) -> Loaded<Fields> {
    let mapping = coerce_mapping(
        payload,
        Place {
            file: LOCAL_FILE,
            label: "settings",
        },
    )?;
    if has(mapping, REQUIRE_SQL_ANALYSIS_KEY) {
        let value = get(mapping, REQUIRE_SQL_ANALYSIS_KEY).unwrap_or(&ConfigValue::Null);
        return Err(shared_policy_failure(value));
    }
    let overrides = setting_overrides(mapping);
    Ok(vec![
        ("settings", load_settings(payload, LOCAL_FILE)?),
        ("setting_overrides", Built::FrozenSet(overrides)),
    ])
}

fn local_fields(payload: &Entries) -> Loaded<Fields> {
    if let Some(recall) = get(payload, RECALL_SECTION) {
        recall_section(recall, RECALL_SECTION)
            .map_err(|message| ConfigFailure::at(LOCAL_FILE, message))?;
    }
    let mut fields: Fields = vec![
        ("target", text_or_none(optional_str(payload, "target")?)),
        ("adapter", text_or_none(optional_str(payload, "adapter")?)),
        (
            "connection",
            Built::Value(optional_mapping(payload, "connection")?),
        ),
        (
            "connections",
            load_connections(get(payload, "connections"), LOCAL_FILE)?,
        ),
    ];
    let targets: Vec<(String, Built)> = load_target_fields(get(payload, "targets"), LOCAL_FILE)?
        .into_iter()
        .map(|(name, target)| (name, local_target_config(target)))
        .collect();
    fields.push(("targets", Built::Dict(targets)));
    fields.extend(local_settings(get(payload, "settings"))?);
    fields.push(("vars", string_mapping(get(payload, "vars"), LOCAL_FILE)?));
    fields.push(("dbt", load_local_dbt(get(payload, "dbt"), LOCAL_FILE)?));
    fields.push((
        "scenario",
        load_scenario(get(payload, "scenario"), LOCAL_FILE)?,
    ));
    Ok(fields)
}

/// `load_local_config`: the validated `LocalConfig`, or the default when the file is absent.
pub(crate) fn local_config(paths: &ConfigPaths) -> Result<Built, LoadFailure> {
    let file = resolve(paths, true)?;
    if !path_of(paths, file).exists() {
        return Ok(object("LocalConfig", Vec::new()));
    }
    let table = load_toml_mapping(paths, file)?;
    let payload = table.as_map().unwrap_or_default();
    Ok(object("LocalConfig", local_fields(payload)?))
}

fn require_setting(value: bool) -> Setting<'static> {
    Setting {
        file_name: PROJECT_CONFIG_FILENAME,
        section: SETTINGS_SECTION,
        key: REQUIRE_SQL_ANALYSIS_KEY,
        value: SettingValue::Bool(value),
    }
}

/// `validate_local_sql_analysis_policy` over (required, overridden, enabled) SQL analysis flags.
pub(crate) fn local_sql_analysis_failure(
    paths: &ConfigPaths,
    flags: (bool, bool, bool),
) -> Option<ConfigFailure> {
    let (required, overridden, enabled) = flags;
    if !required || !overridden || enabled {
        return None;
    }
    let file = match resolve(paths, true) {
        Ok(file) => file,
        Err(failure) => return Some(failure),
    };
    let help = join_helps(&[
        format!(
            "remove {SQL_ANALYSIS_KEY} from [settings] in {LOCAL_CONFIG_FILENAME}; to skip SQL \
             analysis for one run, use `--no-sql-analysis`"
        ),
        setting_help(
            "to allow turning SQL analysis off locally",
            &require_setting(false),
        ),
    ]);
    Some(
        ConfigFailure::at(
            file,
            format!(
                "turns SQL analysis off with [settings] {SQL_ANALYSIS_KEY} = false, which this \
                 project does not allow; {}",
                setting_note(&require_setting(true))
            ),
        )
        .with_help(help),
    )
}

/// One section of `sqlbuild_project.toml`, validated as project loading validates it.
fn project_section<T>(
    paths: &ConfigPaths,
    section: &str,
    load: fn(Option<&ConfigValue>, ConfigFile) -> Loaded<T>,
) -> Result<T, LoadFailure> {
    let file = resolve(paths, false)?;
    let table = load_toml_mapping(paths, file)?;
    let payload = table.as_map().unwrap_or_default();
    Ok(load(get(payload, section), PROJECT_FILE)?)
}

/// `[git] base_ref` of `sqlbuild_project.toml`.
pub(crate) fn git_base_ref(paths: &ConfigPaths) -> Result<Option<String>, LoadFailure> {
    project_section(paths, "git", load_git)
}

/// `[control]` of `sqlbuild_project.toml`.
pub(crate) fn control_config(paths: &ConfigPaths) -> Result<ControlConfig, LoadFailure> {
    project_section(paths, "control", load_control)
}

/// `[review]` of `sqlbuild_project.toml`.
pub(crate) fn review_config(paths: &ConfigPaths) -> Result<ReviewConfig, LoadFailure> {
    project_section(paths, "review", load_review)
}

/// One warning line per configuration file whose `[settings]` still sets `column_contract_mode`.
pub(crate) fn deprecated_setting_warnings(paths: &ConfigPaths) -> Result<Vec<String>, LoadFailure> {
    let mut warnings: Vec<String> = Vec::new();
    for (local, name) in [
        (false, PROJECT_CONFIG_FILENAME),
        (true, LOCAL_CONFIG_FILENAME),
    ] {
        let file = resolve(paths, local)?;
        if !path_of(paths, file).exists() {
            continue;
        }
        if let Some(checked) = legacy_column_contract_mode(&load_toml_mapping(paths, file)?) {
            warnings.push(format!(
                "Warning: settings.column_contract_mode in {name} is deprecated; replace it with \
                 settings.check_declared_columns = {checked}"
            ));
        }
    }
    Ok(warnings)
}

/// The `check_declared_columns` value a valid `[settings] column_contract_mode` alias maps to.
fn legacy_column_contract_mode(table: &ConfigValue) -> Option<bool> {
    let settings = get(table.as_map()?, SETTINGS_SECTION)?.as_map()?;
    let ConfigValue::String(mode) = get(settings, LEGACY_COLUMN_CONTRACT_MODE_KEY)? else {
        return None;
    };
    COLUMN_CONTRACT_MODES
        .iter()
        .find(|(value, _)| value == mode)
        .map(|(_, checked)| *checked)
}
