//! Loaders for `[targets]` and `[connections]`.

use crate::models::ConfigValue;
use crate::project::_helpers::defaults::{
    InheritableField, optional_retention_policy, optional_table_type,
};
use crate::project::_helpers::fields::{
    bool_or_none, coerce_mapping, get, key_name, keyed, object, optional_nullable_bool,
    optional_str, optional_strict_int, optional_text_field, string_mapping, text_or_none,
    value_or_none,
};
use crate::project::_helpers::keys::{
    Place, reject_removed_virtual_keys, reject_unknown_mapping_keys, validate_allowed_keys,
};
use crate::project::_helpers::sections::{fixed_duration_seconds, item_timeout};
use crate::project::_helpers::values::{int_below, is_int};
use crate::project::constants::{
    CLONE_KEYS, CONCURRENCY_KEY, CONFIRMATION_POLICIES, EXECUTION_LIMIT_KEYS, MAX_FULL_ROWS_KEY,
    RETENTION_MATERIALIZATIONS, TARGET_KEYS, UNLIMITED_DIFF_FULL_ROWS, WAREHOUSE_GROUPS,
};
use crate::project::errors::ConfigFailure;
use crate::project::models::{Built, ConfigFile};
use crate::project::types::{Entries, Fields, Loaded};
use crate::recall::constants::RECALL_SECTION;
use crate::recall::main::recall_section::recall_section;

/// Policy fields of a target, with the enum each projects to.
const TARGET_POLICIES: [(&str, &str); 3] = [
    ("table_type_downgrade", "TableTypeDowngradePolicy"),
    ("time_travel_retention_decrease", "RetentionDecreasePolicy"),
    ("missing_migration_origin", "MissingMigrationOriginPolicy"),
];

/// One target being loaded: its name and file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TargetPlace<'a> {
    name: &'a str,
    file: ConfigFile,
}

fn none() -> Built {
    Built::Value(ConfigValue::Null)
}

/// `_load_connections`.
pub(crate) fn load_connections(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(
        payload,
        Place {
            file,
            label: "connections",
        },
    )?;
    let mut loaded: Vec<(String, Built)> = Vec::new();
    for (key, value) in mapping {
        let name = key_name(key);
        if str::trim(name).is_empty() {
            return Err(ConfigFailure::at(
                file,
                "connections contains an empty name",
            ));
        }
        let label = format!("connections.{name}");
        let entries = coerce_mapping(
            Some(value),
            Place {
                file,
                label: &label,
            },
        )?;
        loaded.push((
            name.to_owned(),
            Built::Value(ConfigValue::Map(entries.to_vec())),
        ));
    }
    Ok(Built::Dict(loaded))
}

fn execution_limits(payload: Option<&ConfigValue>, label: &str, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, Place { file, label })?;
    validate_allowed_keys(mapping, &EXECUTION_LIMIT_KEYS, Place { file, label })?;
    let max_models = optional_strict_int(mapping, "max_models")?;
    if max_models
        .as_ref()
        .is_some_and(|number| int_below(number, 1))
    {
        return Err(ConfigFailure::at(
            file,
            format!("{label}.max_models must be >= 1"),
        ));
    }
    let max_duration = optional_str(mapping, "max_duration")?;
    let seconds = match max_duration.as_deref() {
        None => none(),
        Some(text) => match fixed_duration_seconds(text, false)? {
            Some(seconds) => Built::Value(seconds_value(&seconds)),
            None => {
                return Err(ConfigFailure::at(
                    file,
                    format!("{label}.max_duration must be a positive fixed duration such as '30m'"),
                ));
            }
        },
    };
    let mut fields: Fields = vec![
        ("max_models", value_or_none(max_models)),
        ("max_duration", text_or_none(max_duration)),
        ("max_duration_seconds", seconds),
        ("remediation", optional_text_field(mapping, "remediation")?),
    ];
    for (key, seconds_key) in [
        ("test_timeout", "test_timeout_seconds"),
        ("scenario_timeout", "scenario_timeout_seconds"),
    ] {
        let (text, seconds) = item_timeout(mapping, key, (label, file), None)?;
        fields.push((key, text));
        fields.push((seconds_key, seconds));
    }
    Ok(object("ExecutionLimitsConfig", fields))
}

/// Whole seconds as a Python integer.
pub(crate) fn seconds_value(seconds: &str) -> ConfigValue {
    seconds.parse::<i64>().map_or_else(
        |_| ConfigValue::BigInteger(seconds.to_owned()),
        ConfigValue::Integer,
    )
}

/// A target's worker limit, which overrides `[settings] concurrency` for that target.
fn target_concurrency(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<Built> {
    let invalid = || {
        ConfigFailure::at(
            target.file,
            format!(
                "targets.{}.concurrency must be an integer >= 1",
                target.name
            ),
        )
    };
    let value = optional_strict_int(mapping, CONCURRENCY_KEY).map_err(|_| invalid())?;
    if value.as_ref().is_some_and(|number| int_below(number, 1)) {
        return Err(invalid());
    }
    Ok(value_or_none(value))
}

fn target_connection(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<(Built, Built)> {
    match get(mapping, "connection") {
        None => Ok((Built::Value(ConfigValue::Map(Vec::new())), none())),
        Some(value @ ConfigValue::Map(_)) => Ok((Built::Value(value.clone()), none())),
        Some(ConfigValue::String(text)) if !str::trim(text).is_empty() => Ok((
            Built::Value(ConfigValue::Map(Vec::new())),
            Built::Value(ConfigValue::String(str::trim(text).to_owned())),
        )),
        Some(_) => Err(ConfigFailure::at(
            target.file,
            format!(
                "targets.{}.connection must be a non-empty string or mapping",
                target.name
            ),
        )),
    }
}

fn retention_by_materialization(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<Built> {
    let Some(payload @ ConfigValue::Map(_)) = get(mapping, "time_travel_retention") else {
        return Ok(none());
    };
    let label = format!("targets.{}.time_travel_retention", target.name);
    let place = Place {
        file: target.file,
        label: &label,
    };
    let retention = coerce_mapping(Some(payload), place)?;
    validate_allowed_keys(retention, &RETENTION_MATERIALIZATIONS, place)?;
    let mut loaded: Vec<(String, Built)> = Vec::new();
    for materialization in RETENTION_MATERIALIZATIONS {
        let field_label = format!("{label}.{materialization}");
        let policy = optional_retention_policy(
            retention,
            InheritableField {
                key: materialization,
                label: &field_label,
                allow_inherit: false,
            },
            target.file,
        )?;
        if policy != none() {
            loaded.push((materialization.to_owned(), policy));
        }
    }
    Ok(Built::Dict(loaded))
}

fn retention_default(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<Built> {
    if matches!(
        get(mapping, "time_travel_retention"),
        Some(ConfigValue::Map(_))
    ) {
        return Ok(none());
    }
    let label = format!("targets.{}.time_travel_retention", target.name);
    optional_retention_policy(
        mapping,
        InheritableField {
            key: "time_travel_retention",
            label: &label,
            allow_inherit: false,
        },
        target.file,
    )
}

fn target_policies(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<Fields> {
    TARGET_POLICIES
        .into_iter()
        .map(|(key, class)| match get(mapping, key) {
            None => Ok((key, none())),
            Some(ConfigValue::String(text)) if CONFIRMATION_POLICIES.contains(&text.as_str()) => {
                Ok((key, Built::Enum(class, text.clone())))
            }
            Some(_) => Err(ConfigFailure::at(
                target.file,
                format!(
                    "targets.{}.{key} must be 'deny', 'require_confirmation', or 'allow'",
                    target.name
                ),
            )),
        })
        .collect()
}

/// `load_target_diff`.
fn target_diff(payload: Option<&ConfigValue>, target: TargetPlace<'_>) -> Loaded<Built> {
    let label = format!("targets.{}.diff", target.name);
    let entries = match payload {
        None => return Ok(object("TargetDiffConfig", Vec::new())),
        Some(ConfigValue::Map(entries)) => entries,
        Some(_) => {
            return Err(ConfigFailure::at(
                target.file,
                format!("{label} must be a mapping"),
            ));
        }
    };
    let place = Place {
        file: target.file,
        label: &label,
    };
    reject_unknown_mapping_keys(entries, &[MAX_FULL_ROWS_KEY], place)?;
    let rows = match get(entries, MAX_FULL_ROWS_KEY) {
        None => return Ok(object("TargetDiffConfig", Vec::new())),
        Some(ConfigValue::String(text)) if text == UNLIMITED_DIFF_FULL_ROWS => {
            Built::Value(ConfigValue::String(text.clone()))
        }
        Some(value)
            if is_int(value) && !matches!(value, ConfigValue::Bool(_)) && !int_below(value, 1) =>
        {
            Built::Value(value.clone())
        }
        Some(_) => {
            return Err(ConfigFailure::at(
                target.file,
                format!(
                    "{label}.{MAX_FULL_ROWS_KEY} must be an integer >= 1 or '{UNLIMITED_DIFF_FULL_ROWS}'"
                ),
            ));
        }
    };
    Ok(object("TargetDiffConfig", vec![("max_full_rows", rows)]))
}

/// `load_target_warehouses`.
fn target_warehouses(payload: Option<&ConfigValue>, target: TargetPlace<'_>) -> Loaded<Built> {
    let label = format!("targets.{}.warehouses", target.name);
    let entries = match payload {
        None => return Ok(object("TargetWarehousesConfig", Vec::new())),
        Some(ConfigValue::Map(entries)) => entries,
        Some(_) => {
            return Err(ConfigFailure::at(
                target.file,
                format!("{label} must be a mapping"),
            ));
        }
    };
    let place = Place {
        file: target.file,
        label: &label,
    };
    reject_unknown_mapping_keys(entries, &WAREHOUSE_GROUPS, place)?;
    let fields = WAREHOUSE_GROUPS
        .into_iter()
        .map(|group| match get(entries, group) {
            None => Ok((group, none())),
            Some(ConfigValue::String(text)) if !str::trim(text).is_empty() => Ok((
                group,
                Built::Value(ConfigValue::String(str::trim(text).to_owned())),
            )),
            Some(_) => Err(ConfigFailure::at(
                target.file,
                format!("{label}.{group} must be a non-empty warehouse name"),
            )),
        })
        .collect::<Loaded<Fields>>()?;
    Ok(object("TargetWarehousesConfig", fields))
}

fn validate_target_keys(mapping: &Entries, target: TargetPlace<'_>) -> Result<(), ConfigFailure> {
    let label = format!("targets.{}", target.name);
    let place = Place {
        file: target.file,
        label: &label,
    };
    reject_removed_virtual_keys(mapping, &["state", "changes_only"], place)?;
    validate_allowed_keys(mapping, &TARGET_KEYS, place)?;
    if let Some(recall) = get(mapping, RECALL_SECTION) {
        recall_section(recall, &format!("{label}.{RECALL_SECTION}"))
            .map_err(|message| ConfigFailure::at(target.file, message))?;
    }
    Ok(())
}

fn clone_mapping<'a>(mapping: &'a Entries, target: TargetPlace<'_>) -> Loaded<&'a Entries> {
    let label = format!("targets.{}.clone", target.name);
    let place = Place {
        file: target.file,
        label: &label,
    };
    let clone = coerce_mapping(get(mapping, "clone"), place)?;
    validate_allowed_keys(clone, &CLONE_KEYS, place)?;
    Ok(clone)
}

fn text_fields(mapping: &Entries) -> Loaded<Fields> {
    [
        "database",
        "schema",
        "loader_schema",
        "defer_sources_to",
        "defer_clone_from",
    ]
    .into_iter()
    .map(|key| keyed(key, optional_text_field(mapping, key)))
    .collect()
}

fn retention_fields(mapping: &Entries, target: TargetPlace<'_>) -> Loaded<Fields> {
    let table_type_label = format!("targets.{}.default_table_type", target.name);
    let mut fields: Fields = vec![
        (
            "compile_cache",
            bool_or_none(optional_nullable_bool(mapping, "compile_cache")?),
        ),
        ("time_travel_retention", retention_default(mapping, target)?),
        (
            "time_travel_retention_by_materialization",
            retention_by_materialization(mapping, target)?,
        ),
        (
            "owns_time_travel_retention_namespace",
            bool_or_none(optional_nullable_bool(
                mapping,
                "owns_time_travel_retention_namespace",
            )?),
        ),
        (
            "default_table_type",
            optional_table_type(
                mapping,
                InheritableField {
                    key: "default_table_type",
                    label: &table_type_label,
                    allow_inherit: false,
                },
                target.file,
            )?,
        ),
    ];
    fields.extend(target_policies(mapping, target)?);
    Ok(fields)
}

/// One `LocalTargetConfig`'s fields, in the Python loader's evaluation order.
fn local_target(payload: &ConfigValue, target: TargetPlace<'_>) -> Loaded<Fields> {
    let label = format!("targets.{}", target.name);
    let mapping = coerce_mapping(
        Some(payload),
        Place {
            file: target.file,
            label: &label,
        },
    )?;
    validate_target_keys(mapping, target)?;
    let clone = clone_mapping(mapping, target)?;
    let limits_label = format!("targets.{}.execution_limits", target.name);
    let limits = execution_limits(get(mapping, "execution_limits"), &limits_label, target.file)?;
    let (connection, connection_name) = target_connection(mapping, target)?;
    let mut fields: Fields = vec![
        ("connection", connection),
        ("connection_name", connection_name),
        ("vars", string_mapping(get(mapping, "vars"), target.file)?),
    ];
    fields.extend(text_fields(mapping)?);
    fields.extend(retention_fields(mapping, target)?);
    fields.push(("execution_limits", limits));
    fields.push(("diff", target_diff(get(mapping, "diff"), target)?));
    fields.push((
        "warehouses",
        target_warehouses(get(mapping, "warehouses"), target)?,
    ));
    let clone_fields: Fields = CLONE_KEYS
        .into_iter()
        .map(|key| keyed(key, optional_nullable_bool(clone, key).map(bool_or_none)))
        .collect::<Loaded<Fields>>()?;
    fields.push(("clone", object("LocalClonePolicy", clone_fields)));
    fields.push(("concurrency", target_concurrency(mapping, target)?));
    fields.push((
        "catalog_connection",
        optional_text_field(mapping, "catalog_connection")?,
    ));
    Ok(fields)
}

/// `_load_local_targets`: each target's fields by name.
pub(crate) fn load_target_fields(
    payload: Option<&ConfigValue>,
    file: ConfigFile,
) -> Loaded<Vec<(String, Fields)>> {
    let mapping = coerce_mapping(
        payload,
        Place {
            file,
            label: "targets",
        },
    )?;
    mapping
        .iter()
        .map(|(key, value)| {
            let name = key_name(key);
            keyed(
                name.to_owned(),
                local_target(value, TargetPlace { name, file }),
            )
        })
        .collect()
}

fn project_field(key: &'static str, value: Built) -> Option<(&'static str, Built)> {
    let is_none = value == none();
    match key {
        "clone" => Some((key, project_clone(value))),
        "time_travel_retention_by_materialization" if is_none => {
            Some((key, Built::Dict(Vec::new())))
        }
        "owns_time_travel_retention_namespace" => Some((
            key,
            Built::Value(ConfigValue::Bool(
                value == Built::Value(ConfigValue::Bool(true)),
            )),
        )),
        "table_type_downgrade" | "time_travel_retention_decrease" | "missing_migration_origin"
            if is_none =>
        {
            None
        }
        _ => Some((key, value)),
    }
}

fn project_clone(value: Built) -> Built {
    let Built::Object(_, fields) = value else {
        return value;
    };
    object(
        "ClonePolicy",
        fields
            .into_iter()
            .map(|(key, flag)| {
                (
                    key,
                    Built::Value(ConfigValue::Bool(
                        flag == Built::Value(ConfigValue::Bool(true)),
                    )),
                )
            })
            .collect(),
    )
}

/// `_project_target`: a shared target, with the local-only `None`s resolved to defaults.
pub(crate) fn project_target(fields: Fields) -> Built {
    object(
        "TargetConfig",
        fields
            .into_iter()
            .filter_map(|(key, value)| project_field(key, value))
            .collect(),
    )
}

/// A local target.
pub(crate) fn local_target_config(fields: Fields) -> Built {
    object("LocalTargetConfig", fields)
}
