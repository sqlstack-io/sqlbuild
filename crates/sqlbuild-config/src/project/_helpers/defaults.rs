//! Loaders for `[defaults]`, `[path_defaults]` and `[materialization_defaults]`.

use sqlbuild_core::constants::SQL_DIRECTORY;
use crate::models::ConfigValue;
use crate::project::_helpers::durations::parse_duration;
use crate::project::_helpers::fields::{
    bool_or_none, coerce_mapping, get, key_name, keyed, object, optional_batch_size,
    optional_choice, optional_contract_policy, optional_cursor_start, optional_int,
    optional_mapping, optional_non_negative_int, optional_nullable_bool, optional_strict_int,
    optional_templated_bool, optional_text_field, string_sequence, value_or_none,
};
use crate::project::_helpers::help::{change_policy_problem, change_policy_toml_help};
use crate::project::_helpers::keys::{Place, reject_removed_virtual_keys, validate_allowed_keys};
use crate::project::_helpers::sections::item_timeout;
use crate::project::constants::{
    BATCH_CONCURRENCY_KEY, CHANGE_POLICY_KEYS, CONTRACT_POLICIES, DEFAULTS_KEYS, DISABLED_VALUE,
    FUNCTION_DATAFRAMES, GLOB_SEGMENTS, INHERIT_VALUE, MATERIALIZATION_DEFAULT_KEYS,
    MAX_MICROBATCHES_KEY, MODEL_HEADER_KEYS, MODELS_DIRECTORY_NAME, RETENTION_MATERIALIZATIONS,
    TABLE_TYPES, TEMPLATE_OPEN_TOKEN, UNSUPPORTED_GLOB_MARKERS,
};
use crate::project::errors::ConfigFailure;
use crate::project::models::{Built, ConfigFile};
use crate::project::types::{Entries, Fields, Loaded};
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_core::constants::ZERO_DAY_DURATION;

/// A retention or table-type field: where it lives and whether `'inherit'` is accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InheritableField<'a> {
    pub(crate) key: &'a str,
    pub(crate) label: &'a str,
    pub(crate) allow_inherit: bool,
}

fn place(file: ConfigFile, label: &str) -> Place<'_> {
    Place { file, label }
}

fn none() -> Built {
    Built::Value(ConfigValue::Null)
}

/// `_validate_change_policies` for one section of the project file.
fn validate_change_policies(
    mapping: &Entries,
    section: &str,
    file: ConfigFile,
) -> Result<(), ConfigFailure> {
    for key in CHANGE_POLICY_KEYS {
        let Some(value) = get(mapping, key) else {
            continue;
        };
        if matches!(value, ConfigValue::String(text) if text.contains(TEMPLATE_OPEN_TOKEN)) {
            continue;
        }
        if let Some(problem) = change_policy_problem(key, value)? {
            return Err(
                ConfigFailure::at(file, format!("[{section}] {problem}")).with_help(
                    change_policy_toml_help(key, PROJECT_CONFIG_FILENAME, section),
                ),
            );
        }
    }
    Ok(())
}

fn optional_text_fields(mapping: &Entries, keys: &[&'static str]) -> Loaded<Fields> {
    keys.iter()
        .map(|key| keyed(*key, optional_text_field(mapping, key)))
        .collect()
}

fn counted(mapping: &Entries, key: &str) -> Loaded<Built> {
    if get(mapping, key).is_none() {
        return Ok(none());
    }
    Ok(Built::Value(optional_int(mapping, key, 1)?))
}

fn cursor_fields(mapping: &Entries) -> Loaded<Fields> {
    let mut fields: Fields = vec![
        (
            "full_refresh",
            bool_or_none(optional_nullable_bool(mapping, "full_refresh")?),
        ),
        (
            "append_cursor_inclusive",
            optional_templated_bool(mapping, "append_cursor_inclusive")?,
        ),
        (
            "cursor_start",
            optional_cursor_start(mapping, "cursor_start")?,
        ),
        ("cursor_end", optional_cursor_start(mapping, "cursor_end")?),
    ];
    fields.extend(optional_text_fields(
        mapping,
        &[
            "cursor_start_max_ahead",
            "cursor_start_max_action",
            "cursor_future_max_distance",
            "cursor_future_action",
            "lookback",
        ],
    )?);
    fields.push(("batch_size", optional_batch_size(mapping, "batch_size")?));
    fields.push((
        "batch_concurrency",
        counted(mapping, BATCH_CONCURRENCY_KEY)?,
    ));
    fields.push(("max_microbatches", counted(mapping, MAX_MICROBATCHES_KEY)?));
    Ok(fields)
}

fn sequences(mapping: &Entries, file: ConfigFile) -> Loaded<[Built; 3]> {
    let load = |key: &str, label: &str| {
        string_sequence(get(mapping, key), place(file, label)).map(Built::Tuple)
    };
    Ok([
        load(
            "row_diff_exclude_columns",
            "defaults.row_diff_exclude_columns",
        )?,
        load("merge_exclude_columns", "defaults.merge_exclude_columns")?,
        load("tags", "defaults.tags")?,
    ])
}

/// `_load_defaults`.
pub(crate) fn load_defaults(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "defaults"))?;
    reject_removed_virtual_keys(mapping, &["run_despite_unchanged"], place(file, "defaults"))?;
    validate_allowed_keys(mapping, &DEFAULTS_KEYS, place(file, "[defaults]"))?;
    validate_change_policies(mapping, "defaults", file)?;
    let [row_diff_exclude, merge_exclude, tags] = sequences(mapping, file)?;
    let tolerances = Built::Value(optional_mapping(mapping, "row_diff_tolerances")?);
    let mut fields = optional_text_fields(
        mapping,
        &[
            "materialized",
            "database",
            "schema",
            "seed_database",
            "seed_schema",
            "function_database",
            "function_schema",
        ],
    )?;
    fields.push((
        "contract",
        optional_contract_policy(mapping, "contract", &CONTRACT_POLICIES)?,
    ));
    fields.push((
        "function_dataframe",
        Built::Value(ConfigValue::String(optional_choice(
            mapping,
            "function_dataframe",
            &FUNCTION_DATAFRAMES,
            FUNCTION_DATAFRAMES[0],
        )?)),
    ));
    fields.extend(optional_text_fields(
        mapping,
        &[
            "incremental_strategy",
            "incremental_mode",
            "microbatch_strategy",
            "cursor_watermark_mode",
        ],
    )?);
    fields.push(("merge_exclude_columns", merge_exclude));
    fields.extend(cursor_fields(mapping)?);
    fields.extend(optional_text_fields(
        mapping,
        &["unaccounted_partition_policy", "replay_on_change"],
    )?);
    fields.extend([
        ("row_diff_exclude_columns", row_diff_exclude),
        ("row_diff_tolerances", tolerances),
        (
            "row_diff_sample_rows",
            value_or_none(optional_non_negative_int(mapping, "row_diff_sample_rows")?),
        ),
        (
            "row_diff_sample_seed",
            value_or_none(optional_strict_int(mapping, "row_diff_sample_seed")?),
        ),
        ("tags", tags),
    ]);
    let (max_duration, _) = item_timeout(mapping, "max_duration", ("defaults", file), None)?;
    fields.push(("max_duration", max_duration));
    Ok(object("DefaultsConfig", fields))
}

fn invalid_path_key(file: ConfigFile, path_key: &str, problem: &str) -> ConfigFailure {
    ConfigFailure::at(file, format!("path_defaults['{path_key}'] {problem}"))
}

/// `_normalize_path_default_key`.
fn normalize_path_key(path_key: &str, file: ConfigFile) -> Loaded<String> {
    let normalized = str::trim(path_key);
    if normalized.is_empty() {
        return Err(ConfigFailure::at(
            file,
            "path_defaults contains an empty path key",
        ));
    }
    if normalized.starts_with('/') {
        return Err(invalid_path_key(
            file,
            path_key,
            "is invalid. Use model-relative paths without a leading slash, for example \
             'staging' or 'staging/nested'.",
        ));
    }
    if normalized.ends_with('/') {
        return Err(invalid_path_key(
            file,
            path_key,
            "is invalid. Remove the trailing slash and use a model-relative path such as \
             'staging' or 'staging/nested'.",
        ));
    }
    let parts: Vec<&str> = normalized.split('/').collect();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(invalid_path_key(
            file,
            path_key,
            "is invalid. Path defaults cannot contain empty path segments.",
        ));
    }
    if parts[0] == MODELS_DIRECTORY_NAME
        || (parts[0] == SQL_DIRECTORY && parts.get(1) == Some(&MODELS_DIRECTORY_NAME))
    {
        return Err(invalid_path_key(
            file,
            path_key,
            "uses a redundant 'sql/models/' prefix. Use a model-relative path such as 'staging' \
             or 'staging/nested'.",
        ));
    }
    let unsupported: Vec<&str> = parts
        .iter()
        .copied()
        .filter(|part| {
            !GLOB_SEGMENTS.contains(part) && part.contains(UNSUPPORTED_GLOB_MARKERS.as_slice())
        })
        .collect();
    if !unsupported.is_empty() {
        return Err(invalid_path_key(
            file,
            path_key,
            &format!(
                "contains unsupported glob segment(s): {}. Use '*' or '**' as complete path segments.",
                unsupported.join(", ")
            ),
        ));
    }
    Ok(parts.join("/"))
}

fn validate_path_default_tags(
    entry: &Entries,
    path_key: &str,
    file: ConfigFile,
) -> Result<(), ConfigFailure> {
    match get(entry, "tags") {
        None => Ok(()),
        Some(ConfigValue::List(items)) => {
            if items
                .iter()
                .all(|item| matches!(item, ConfigValue::String(_)))
            {
                Ok(())
            } else {
                Err(ConfigFailure::at(
                    file,
                    format!("path_defaults['{path_key}'].tags entries must be strings"),
                ))
            }
        }
        Some(_) => Err(ConfigFailure::at(
            file,
            format!("path_defaults['{path_key}'].tags must be a list"),
        )),
    }
}

fn path_default_entry<'a>(
    path_key: &str,
    value: &'a ConfigValue,
    file: ConfigFile,
) -> Loaded<&'a Entries> {
    let ConfigValue::Map(entry) = value else {
        return Err(invalid_path_key(file, path_key, "must be a mapping"));
    };
    reject_removed_virtual_keys(
        entry,
        &["run_despite_unchanged"],
        place(file, "path_defaults"),
    )?;
    let label = format!("path_defaults['{path_key}']");
    validate_allowed_keys(entry, &MODEL_HEADER_KEYS, place(file, &label))?;
    validate_change_policies(entry, &format!("path_defaults.\"{path_key}\""), file)?;
    validate_path_default_tags(entry, path_key, file)?;
    Ok(entry)
}

/// `_load_path_defaults`: entries by normalized model-relative path.
pub(crate) fn load_path_defaults(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "path_defaults"))?;
    let mut loaded: Vec<(String, Built)> = Vec::new();
    for (key, value) in mapping {
        let path_key = key_name(key);
        let entry = path_default_entry(path_key, value, file)?;
        let normalized = normalize_path_key(path_key, file)?;
        if loaded.iter().any(|(existing, _)| *existing == normalized) {
            return Err(ConfigFailure::at(
                file,
                format!(
                    "path_defaults defines both '{path_key}' and another key that normalize to \
                     '{normalized}'. Use only the canonical model-relative form without a \
                     leading 'models/' prefix."
                ),
            ));
        }
        loaded.push((normalized, Built::Value(ConfigValue::Map(entry.to_vec()))));
    }
    Ok(Built::Dict(loaded))
}

fn retention(desired_days: Option<&str>) -> Built {
    match desired_days {
        None => object(
            "AuthoredTimeTravelRetention",
            vec![("unmanaged", Built::Value(ConfigValue::Bool(true)))],
        ),
        Some(days) => object(
            "AuthoredTimeTravelRetention",
            vec![("desired_days", Built::Value(day_count(days)))],
        ),
    }
}

fn day_count(days: &str) -> ConfigValue {
    days.parse::<i64>().map_or_else(
        |_| ConfigValue::BigInteger(days.to_owned()),
        ConfigValue::Integer,
    )
}

/// `_optional_retention_policy`.
pub(crate) fn optional_retention_policy(
    mapping: &Entries,
    field: InheritableField<'_>,
    file: ConfigFile,
) -> Loaded<Built> {
    let label = field.label;
    let Some(value) = get(mapping, field.key) else {
        return Ok(none());
    };
    let ConfigValue::String(text) = value else {
        return Err(ConfigFailure::at(
            file,
            format!("{label} must be a whole-day string like '7d'"),
        ));
    };
    if field.allow_inherit && text == INHERIT_VALUE {
        return Ok(none());
    }
    if text == DISABLED_VALUE {
        return Ok(retention(None));
    }
    if text == ZERO_DAY_DURATION {
        return Ok(retention(Some("0")));
    }
    match parse_duration(text)?.filter(super::durations::Duration::is_whole_days) {
        Some(duration) => Ok(retention(Some(&duration.days()))),
        None => Err(ConfigFailure::at(
            file,
            format!(
                "{label} must be a whole-day string like '7d'{}",
                if field.allow_inherit {
                    ", 'inherit', or 'disabled'"
                } else {
                    " or 'disabled'"
                }
            ),
        )),
    }
}

/// `_optional_table_type`.
pub(crate) fn optional_table_type(
    mapping: &Entries,
    field: InheritableField<'_>,
    file: ConfigFile,
) -> Loaded<Built> {
    match get(mapping, field.key) {
        None => Ok(none()),
        Some(ConfigValue::String(text)) if field.allow_inherit && text == INHERIT_VALUE => {
            Ok(none())
        }
        Some(ConfigValue::String(text)) if TABLE_TYPES.contains(&text.as_str()) => {
            Ok(Built::Enum("TableType", text.clone()))
        }
        Some(_) => Err(ConfigFailure::at(
            file,
            format!(
                "{} must be 'permanent' or 'transient'{}",
                field.label,
                if field.allow_inherit {
                    ", or 'inherit'"
                } else {
                    ""
                }
            ),
        )),
    }
}

fn materialization_defaults(
    mapping: &Entries,
    materialization: &str,
    file: ConfigFile,
) -> Loaded<Built> {
    let label = format!("materialization_defaults.{materialization}");
    let entry = coerce_mapping(get(mapping, materialization), place(file, &label))?;
    validate_allowed_keys(entry, &MATERIALIZATION_DEFAULT_KEYS, place(file, &label))?;
    let retention_label = format!("{label}.time_travel_retention");
    let table_type_label = format!("{label}.table_type");
    Ok(object(
        "MaterializationRetentionDefaults",
        vec![
            (
                "time_travel_retention",
                optional_retention_policy(
                    entry,
                    InheritableField {
                        key: "time_travel_retention",
                        label: &retention_label,
                        allow_inherit: true,
                    },
                    file,
                )?,
            ),
            (
                "table_type",
                optional_table_type(
                    entry,
                    InheritableField {
                        key: "table_type",
                        label: &table_type_label,
                        allow_inherit: true,
                    },
                    file,
                )?,
            ),
        ],
    ))
}

/// `_load_materialization_defaults`.
pub(crate) fn load_materialization_defaults(
    payload: Option<&ConfigValue>,
    file: ConfigFile,
) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "materialization_defaults"))?;
    validate_allowed_keys(
        mapping,
        &RETENTION_MATERIALIZATIONS,
        place(file, "materialization_defaults"),
    )?;
    let fields = RETENTION_MATERIALIZATIONS
        .into_iter()
        .map(|materialization| {
            keyed(
                materialization,
                materialization_defaults(mapping, materialization, file),
            )
        })
        .collect::<Loaded<Fields>>()?;
    Ok(object("MaterializationDefaultsConfig", fields))
}
