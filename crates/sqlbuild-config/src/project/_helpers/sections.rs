//! Loaders for the scalar sections of the project and local configuration files.

use crate::models::ConfigValue;
use crate::project::_helpers::durations::parse_duration;
use crate::project::_helpers::fields::{
    coerce_mapping, get, has, insert_entry, key_name, keyed, member, object, object_mapping,
    optional_bool, optional_choice, optional_int, optional_nullable_int, optional_str,
    optional_text_field, string_mapping, string_sequence, text_or_none, value_or_none,
};
use crate::project::_helpers::keys::{Place, reject_removed_virtual_keys, validate_allowed_keys};
use crate::project::_helpers::targets::seconds_value;
use crate::project::_helpers::values::{int_below, python_equal};
use crate::project::constants::{
    AUDIT_SEVERITIES, CHECK_DECLARED_COLUMNS_KEY, COLLECTION_RENDERINGS, COLUMN_CONTRACT_MODES,
    CONCURRENCY_KEY, DBT_DEFER_CLONE_KEY, DBT_KEYS, DBT_REMOVED_KEYS, DBT_REUSE_KEYS,
    DEFAULT_IN_USE_WINDOW, DEFAULT_QUERY_ARTIFACT_TTL, FUTURE_CURSOR_ACTIONS,
    LEGACY_COLUMN_CONTRACT_MODE_KEY, LEGACY_CONCURRENCY_KEY, LEGACY_SQL_VALIDATION_KEY,
    LOCAL_DBT_KEYS, MAX_BATCHES_KEY, MICROBATCH_LIMIT_ACTIONS, OLD_NAME_VIEWS_KEY,
    QUERY_CHANGE_TRACKING_KEY, REQUIRE_SQL_ANALYSIS_KEY, REVIEW_DOWNSTREAM_POLICIES, REVIEW_KEYS,
    RUN_NAMESPACE_KEY, RUN_NAMESPACE_MAX_LENGTH, SCENARIO_KEYS, SETTINGS_KEYS,
    SNAPSHOT_FULL_REFRESH_POLICIES, SNAPSHOT_KEYS, SNAPSHOT_LIMIT_KEYS,
    SNAPSHOT_SCHEMA_CHANGE_POLICIES, SQL_ANALYSIS_KEY, UNACCOUNTED_PARTITION_POLICIES,
    USD_PER_CREDIT_KEY, ZERO_FIXED_DURATION, ZERO_FLOAT,
};
use crate::project::constants::{
    CONTROL_KEYS, MIN_SYNC_INTERVAL_SECONDS, SYNC_INTERVAL_HELP, SYNC_INTERVAL_OFF,
};
use crate::project::constants::{
    DEFAULT_TEST_TIMEOUT, DEFAULT_UNIT_TEST_MAX_REAL_MODELS, DELTA_COMPILE_KEY, DISABLED_VALUE,
    SCENARIO_TIMEOUT_KEY, TEST_TIMEOUT_KEY, UNIT_TEST_MAX_REAL_MODELS_KEY,
};
use crate::project::errors::ConfigFailure;
use crate::project::models::{Built, ConfigFile, ControlConfig, ReviewConfig};
use crate::project::types::{Entries, Fields, Loaded};
use sqlbuild_core::constants::ZERO_DAY_DURATION;

fn place(file: ConfigFile, label: &str) -> Place<'_> {
    Place { file, label }
}

/// `_fixed_duration_seconds`: whole seconds of a fixed duration, or `None`.
pub(crate) fn fixed_duration_seconds(value: &str, allow_zero: bool) -> Loaded<Option<String>> {
    if allow_zero && value == ZERO_FIXED_DURATION {
        return Ok(Some("0".to_owned()));
    }
    Ok(parse_duration(value)?
        .filter(|duration| !duration.has_calendar_component())
        .map(|duration| duration.fixed_seconds().decimal()))
}

/// A per-item execution timeout: its text and whole seconds, `None` seconds when disabled.
pub(crate) fn item_timeout(
    mapping: &Entries,
    key: &str,
    (label, file): (&str, ConfigFile),
    default: Option<&str>,
) -> Loaded<(Built, Built)> {
    let Some(text) = optional_str(mapping, key)?.or_else(|| default.map(str::to_owned)) else {
        return Ok((
            Built::Value(ConfigValue::Null),
            Built::Value(ConfigValue::Null),
        ));
    };
    if text == DISABLED_VALUE {
        return Ok((
            Built::Value(ConfigValue::String(text)),
            Built::Value(ConfigValue::Null),
        ));
    }
    match fixed_duration_seconds(&text, false)? {
        Some(seconds) => Ok((
            Built::Value(ConfigValue::String(text)),
            Built::Value(seconds_value(&seconds)),
        )),
        None => Err(ConfigFailure::at(
            file,
            format!(
                "{label}.{key} must be a positive fixed duration such as '30s' or '10m', or \
                 'disabled'"
            ),
        )),
    }
}

fn real_model_limit(mapping: &Entries) -> Loaded<ConfigValue> {
    let value = optional_int(
        mapping,
        UNIT_TEST_MAX_REAL_MODELS_KEY,
        DEFAULT_UNIT_TEST_MAX_REAL_MODELS,
    )?;
    if int_below(&value, 1) {
        return Err(ConfigFailure::plain(format!(
            "settings.{UNIT_TEST_MAX_REAL_MODELS_KEY} must be >= 1"
        )));
    }
    Ok(value)
}

fn settings_mapping(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<&Entries> {
    let mapping = coerce_mapping(payload, place(file, "settings"))?;
    reject_removed_virtual_keys(
        mapping,
        &["virtual_environments", "changes_only"],
        place(file, "settings"),
    )?;
    let allowed: Vec<&str> = SETTINGS_KEYS
        .into_iter()
        .chain([
            LEGACY_CONCURRENCY_KEY,
            LEGACY_SQL_VALIDATION_KEY,
            LEGACY_COLUMN_CONTRACT_MODE_KEY,
        ])
        .collect();
    validate_allowed_keys(mapping, &allowed, place(file, "settings"))?;
    if has(mapping, CONCURRENCY_KEY) && has(mapping, LEGACY_CONCURRENCY_KEY) {
        return Err(ConfigFailure::plain(
            "settings cannot define both 'concurrency' and legacy 'max_concurrency'",
        ));
    }
    if has(mapping, CHECK_DECLARED_COLUMNS_KEY) && has(mapping, LEGACY_COLUMN_CONTRACT_MODE_KEY) {
        return Err(ConfigFailure::plain(
            "settings cannot define both 'check_declared_columns' and deprecated \
             'column_contract_mode'; keep only check_declared_columns",
        ));
    }
    if let (Some(current), Some(legacy)) = (
        get(mapping, SQL_ANALYSIS_KEY),
        get(mapping, LEGACY_SQL_VALIDATION_KEY),
    ) && !python_equal(current, legacy)
    {
        return Err(ConfigFailure::plain(
            "settings.sql_analysis conflicts with legacy settings.sql_validation",
        ));
    }
    Ok(mapping)
}

/// `check_declared_columns`, or what the deprecated `column_contract_mode` it replaces means.
fn check_declared_columns(mapping: &Entries) -> Loaded<bool> {
    if !has(mapping, LEGACY_COLUMN_CONTRACT_MODE_KEY) {
        return optional_bool(mapping, CHECK_DECLARED_COLUMNS_KEY, true);
    }
    let mode = match get(mapping, LEGACY_COLUMN_CONTRACT_MODE_KEY) {
        Some(ConfigValue::String(text)) => Some(text.as_str()),
        _ => None,
    };
    COLUMN_CONTRACT_MODES
        .iter()
        .find(|(name, _)| Some(*name) == mode)
        .map(|(_, checked)| *checked)
        .ok_or_else(|| {
            ConfigFailure::plain(
                "settings.column_contract_mode must be one of: implicit, explicit. It is \
                 deprecated; set this in sqlbuild_project.toml instead, with false to leave \
                 declared columns unchecked:\n            [settings]\n            \
                 check_declared_columns = true",
            )
        })
}

fn unaccounted_partition_policy(mapping: &Entries) -> Loaded<String> {
    let policy = optional_str(mapping, "microbatch_unaccounted_partition_policy")?
        .unwrap_or_else(|| "synthesize".to_owned());
    if !UNACCOUNTED_PARTITION_POLICIES.contains(&policy.as_str()) {
        return Err(ConfigFailure::plain(format!(
            "settings.microbatch_unaccounted_partition_policy must be one of: {}",
            UNACCOUNTED_PARTITION_POLICIES.join(", ")
        )));
    }
    Ok(policy)
}

fn concurrency(mapping: &Entries) -> Loaded<ConfigValue> {
    let key = if has(mapping, LEGACY_CONCURRENCY_KEY) {
        LEGACY_CONCURRENCY_KEY
    } else {
        CONCURRENCY_KEY
    };
    let value = optional_int(mapping, key, 1)?;
    if int_below(&value, 1) {
        return Err(ConfigFailure::plain("settings.concurrency must be >= 1"));
    }
    Ok(value)
}

fn default_audit_severity(mapping: &Entries) -> Loaded<Built> {
    match optional_str(mapping, "default_audit_severity")? {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(severity) if AUDIT_SEVERITIES.contains(&severity.as_str()) => {
            Ok(member("AuditSeverity", &severity))
        }
        Some(_) => Err(ConfigFailure::plain(format!(
            "settings.default_audit_severity must be one of: {}",
            AUDIT_SEVERITIES.join(", ")
        ))),
    }
}

/// `_load_settings`.
pub(crate) fn load_settings(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = settings_mapping(payload, file)?;
    let analysis_key = if has(mapping, SQL_ANALYSIS_KEY) {
        SQL_ANALYSIS_KEY
    } else {
        LEGACY_SQL_VALIDATION_KEY
    };
    let fields: Fields = vec![
        (
            "sql_analysis",
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                analysis_key,
                true,
            )?)),
        ),
        (
            "require_sql_analysis",
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                REQUIRE_SQL_ANALYSIS_KEY,
                false,
            )?)),
        ),
        (
            "query_change_tracking",
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                QUERY_CHANGE_TRACKING_KEY,
                true,
            )?)),
        ),
        (
            "check_declared_columns",
            Built::Value(ConfigValue::Bool(check_declared_columns(mapping)?)),
        ),
        (
            "auto_load_sources",
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                "auto_load_sources",
                true,
            )?)),
        ),
        (
            "microbatch_concurrency",
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                "microbatch_concurrency",
                false,
            )?)),
        ),
        (
            "microbatch_unaccounted_partition_policy",
            Built::Value(ConfigValue::String(unaccounted_partition_policy(mapping)?)),
        ),
        ("concurrency", Built::Value(concurrency(mapping)?)),
        (
            "table_promotion_mode",
            text_or_none(optional_str(mapping, "table_promotion_mode")?),
        ),
        ("default_audit_severity", default_audit_severity(mapping)?),
        (
            "default_audit_run_scope",
            text_or_none(optional_str(mapping, "default_audit_run_scope")?),
        ),
        (
            UNIT_TEST_MAX_REAL_MODELS_KEY,
            Built::Value(real_model_limit(mapping)?),
        ),
        (
            DELTA_COMPILE_KEY,
            Built::Value(ConfigValue::Bool(optional_bool(
                mapping,
                DELTA_COMPILE_KEY,
                true,
            )?)),
        ),
    ];
    let mut fields = fields;
    for (key, seconds_key, default) in [
        (
            TEST_TIMEOUT_KEY,
            "test_timeout_seconds",
            Some(DEFAULT_TEST_TIMEOUT),
        ),
        (SCENARIO_TIMEOUT_KEY, "scenario_timeout_seconds", None),
    ] {
        let (text, seconds) = item_timeout(mapping, key, ("settings", file), default)?;
        fields.push((key, text));
        fields.push((seconds_key, seconds));
    }
    Ok(object("SettingsConfig", fields))
}

/// The `[settings]` keys a local file sets, under their current names.
pub(crate) fn setting_overrides(mapping: &Entries) -> Vec<String> {
    let mut overrides: Vec<String> = Vec::new();
    for (key, _) in mapping {
        let name = match key_name(key) {
            LEGACY_CONCURRENCY_KEY => CONCURRENCY_KEY,
            LEGACY_SQL_VALIDATION_KEY => SQL_ANALYSIS_KEY,
            LEGACY_COLUMN_CONTRACT_MODE_KEY => CHECK_DECLARED_COLUMNS_KEY,
            other if SETTINGS_KEYS.contains(&other) => other,
            _ => continue,
        };
        if !overrides.iter().any(|existing| existing == name) {
            overrides.push(name.to_owned());
        }
    }
    overrides
}

fn cursor_limit(
    mapping: &Entries,
    keys: (&str, &str),
    label: &str,
) -> Loaded<(Option<String>, Built)> {
    let limit = optional_str(mapping, keys.0)?;
    let invalid = match limit.as_deref() {
        Some(text) if text != ZERO_DAY_DURATION => parse_duration(text)?.is_none(),
        _ => false,
    };
    if invalid {
        return Err(ConfigFailure::plain(format!(
            "{label}.{} must be a duration like 0d, 7d, 12h, or 1mo",
            keys.0
        )));
    }
    let action = optional_str(mapping, keys.1)?;
    let action = action.as_deref().unwrap_or("error");
    if !FUTURE_CURSOR_ACTIONS.contains(&action) {
        return Err(ConfigFailure::plain(format!(
            "{label}.action must be one of: cap, error"
        )));
    }
    Ok((limit, member("FutureCursorAction", action)))
}

/// `_load_cursors`.
pub(crate) fn load_cursors(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "cursors"))?;
    validate_allowed_keys(mapping, &["future", "start"], place(file, "cursors"))?;
    let future = coerce_mapping(get(mapping, "future"), place(file, "cursors.future"))?;
    validate_allowed_keys(
        future,
        &["max_distance", "action"],
        place(file, "cursors.future"),
    )?;
    let (max_distance, action) =
        cursor_limit(future, ("max_distance", "action"), "cursors.future")?;
    let start = coerce_mapping(get(mapping, "start"), place(file, "cursors.start"))?;
    validate_allowed_keys(
        start,
        &["max_ahead", "action"],
        place(file, "cursors.start"),
    )?;
    let (max_ahead, start_action) = cursor_limit(start, ("max_ahead", "action"), "cursors.start")?;
    Ok(object(
        "CursorsConfig",
        vec![
            (
                "future",
                object(
                    "FutureCursorsConfig",
                    vec![
                        ("max_distance", text_or_none(max_distance)),
                        ("action", action),
                    ],
                ),
            ),
            (
                "start",
                object(
                    "StartCursorsConfig",
                    vec![
                        ("max_ahead", text_or_none(max_ahead)),
                        ("action", start_action),
                    ],
                ),
            ),
        ],
    ))
}

fn positive_max_batches(limits: &Entries) -> Loaded<Option<ConfigValue>> {
    if !has(limits, MAX_BATCHES_KEY) {
        return Ok(None);
    }
    let invalid =
        || ConfigFailure::plain("microbatches.limits.max_batches must be a positive integer");
    if matches!(get(limits, MAX_BATCHES_KEY), Some(ConfigValue::Bool(_))) {
        return Err(invalid());
    }
    let value = optional_int(limits, MAX_BATCHES_KEY, 0)?;
    if int_below(&value, 1) {
        return Err(invalid());
    }
    Ok(Some(value))
}

/// `_load_microbatches`.
pub(crate) fn load_microbatches(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "microbatches"))?;
    validate_allowed_keys(mapping, &["limits"], place(file, "microbatches"))?;
    let limits = coerce_mapping(get(mapping, "limits"), place(file, "microbatches.limits"))?;
    validate_allowed_keys(
        limits,
        &[MAX_BATCHES_KEY, "action"],
        place(file, "microbatches.limits"),
    )?;
    let max_batches = positive_max_batches(limits)?;
    let action = optional_str(limits, "action")?.unwrap_or_else(|| "error".to_owned());
    if !MICROBATCH_LIMIT_ACTIONS.contains(&action.as_str()) {
        return Err(ConfigFailure::plain(
            "microbatches.limits.action must be one of: error, warn",
        ));
    }
    Ok(object(
        "MicrobatchesConfig",
        vec![(
            "limits",
            object(
                "MicrobatchLimitsConfig",
                vec![
                    ("max_batches", value_or_none(max_batches)),
                    ("action", member("MicrobatchLimitAction", &action)),
                ],
            ),
        )],
    ))
}

/// `_load_migrations`.
pub(crate) fn load_migrations(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "migrations"))?;
    validate_allowed_keys(mapping, &[OLD_NAME_VIEWS_KEY], place(file, "migrations"))?;
    let views = match get(mapping, OLD_NAME_VIEWS_KEY) {
        None => return Ok(object("MigrationsConfig", Vec::new())),
        Some(ConfigValue::Bool(false)) => Built::Value(ConfigValue::Null),
        Some(ConfigValue::String(text)) if parse_duration(str::trim(text))?.is_some() => {
            Built::Value(ConfigValue::String(str::trim(text).to_owned()))
        }
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "migrations.old_name_views must be a positive duration such as '30d', \
                 or false to disable compatibility views at old model names",
            ));
        }
    };
    Ok(object("MigrationsConfig", vec![("old_name_views", views)]))
}

/// `_load_scopes` and `_load_references`: one boolean policy that defaults to on.
pub(crate) fn load_policy_flag(
    payload: Option<&ConfigValue>,
    section: (&'static str, &'static str, &'static str),
    file: ConfigFile,
) -> Loaded<Built> {
    let (label, key, class) = section;
    let mapping = coerce_mapping(payload, place(file, label))?;
    validate_allowed_keys(mapping, &[key], place(file, label))?;
    Ok(object(
        class,
        vec![(
            key,
            Built::Value(ConfigValue::Bool(optional_bool(mapping, key, true)?)),
        )],
    ))
}

fn is_positive_number(value: &ConfigValue) -> bool {
    match value {
        ConfigValue::Integer(number) => *number > 0,
        ConfigValue::BigInteger(text) => !text.starts_with('-'),
        ConfigValue::Float(number) => number.is_finite() && *number > ZERO_FLOAT,
        _ => false,
    }
}

/// `_load_cost`.
pub(crate) fn load_cost(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "cost"))?;
    validate_allowed_keys(mapping, &[USD_PER_CREDIT_KEY], place(file, "cost"))?;
    let raw = match get(mapping, USD_PER_CREDIT_KEY) {
        None => return Ok(object("CostConfig", Vec::new())),
        Some(
            raw @ (ConfigValue::Integer(_) | ConfigValue::BigInteger(_) | ConfigValue::Float(_)),
        ) => raw,
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "cost.usd_per_credit must be a number",
            ));
        }
    };
    if !is_positive_number(raw) {
        return Err(ConfigFailure::at(
            file,
            "cost.usd_per_credit must be a finite number greater than zero",
        ));
    }
    Ok(object(
        "CostConfig",
        vec![
            ("usd_per_credit", Built::Decimal(raw.clone())),
            (
                "usd_per_credit_is_default",
                Built::Value(ConfigValue::Bool(false)),
            ),
        ],
    ))
}

/// `_load_constants`.
pub(crate) fn load_constants(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "constants"))?;
    validate_allowed_keys(mapping, &["collection_rendering"], place(file, "constants"))?;
    let rendering = match get(mapping, "collection_rendering") {
        None => COLLECTION_RENDERINGS[0],
        Some(ConfigValue::String(text)) => text.as_str(),
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "constants.collection_rendering must be a string",
            ));
        }
    };
    if !COLLECTION_RENDERINGS.contains(&rendering) {
        return Err(ConfigFailure::at(
            file,
            format!(
                "constants.collection_rendering must be one of: {}",
                COLLECTION_RENDERINGS.join(", ")
            ),
        ));
    }
    Ok(object(
        "ConstantsConfig",
        vec![(
            "collection_rendering",
            member("CollectionRendering", rendering),
        )],
    ))
}

/// `_load_diff`.
pub(crate) fn load_diff(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "diff"))?;
    validate_allowed_keys(mapping, &["query_artifact_ttl"], place(file, "diff"))?;
    let ttl = optional_str(mapping, "query_artifact_ttl")?
        .unwrap_or_else(|| DEFAULT_QUERY_ARTIFACT_TTL.to_owned());
    if fixed_duration_seconds(&ttl, false)?.is_none() {
        return Err(ConfigFailure::at(
            file,
            "diff.query_artifact_ttl must be a positive fixed duration such as '24h'",
        ));
    }
    Ok(object(
        "DiffConfig",
        vec![("query_artifact_ttl", Built::Value(ConfigValue::String(ttl)))],
    ))
}

/// `[git]`: the `base_ref` that `--since @base` compares against, when set.
pub(crate) fn load_git(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Option<String>> {
    let mapping = coerce_mapping(payload, place(file, "git"))?;
    validate_allowed_keys(mapping, &["base_ref"], place(file, "git"))?;
    optional_str(mapping, "base_ref").map_err(|_| {
        ConfigFailure::at(
            file,
            "git.base_ref must be a non-empty git ref such as 'origin/main'",
        )
    })
}

/// A safety sync interval in seconds, `0` when off; the error completes "<setting> ...".
pub(crate) fn sync_interval(text: &str) -> Result<u64, &'static str> {
    if SYNC_INTERVAL_OFF.contains(&text) {
        return Ok(0);
    }
    let seconds: u64 = match fixed_duration_seconds(text, false) {
        Ok(Some(seconds)) => seconds.parse().map_err(|_| SYNC_INTERVAL_HELP)?,
        Ok(None) | Err(_) => return Err(SYNC_INTERVAL_HELP),
    };
    if seconds < MIN_SYNC_INTERVAL_SECONDS {
        return Err(SYNC_INTERVAL_HELP);
    }
    Ok(seconds)
}

/// `[control]`: where a signed-in build asks for a sync, and how often a location syncs anyway.
pub(crate) fn load_control(
    payload: Option<&ConfigValue>,
    file: ConfigFile,
) -> Loaded<ControlConfig> {
    let mapping = coerce_mapping(payload, place(file, "control"))?;
    validate_allowed_keys(mapping, &CONTROL_KEYS, place(file, "control"))?;
    let server = optional_str(mapping, "server").map_err(|_| {
        ConfigFailure::at(
            file,
            "control.server must be an HTTPS server origin such as 'https://sqlbuild.example.com'",
        )
    })?;
    let location = optional_str(mapping, "location").map_err(|_| {
        ConfigFailure::at(file, "control.location must be a non-empty location name")
    })?;
    let sync_at_least_every_seconds = match get(mapping, "sync_at_least_every") {
        None => None,
        Some(value) => Some(
            match value {
                ConfigValue::String(text) => sync_interval(text),
                _ => Err(SYNC_INTERVAL_HELP),
            }
            .map_err(|help| {
                ConfigFailure::at(file, format!("control.sync_at_least_every {help}"))
            })?,
        ),
    };
    Ok(ControlConfig {
        server,
        location,
        sync_at_least_every_seconds,
    })
}

/// `[review]`: the defaults of `sqb review`, validated so a typo fails like any other section.
pub(crate) fn load_review(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<ReviewConfig> {
    let mapping = coerce_mapping(payload, place(file, "review"))?;
    validate_allowed_keys(mapping, &REVIEW_KEYS, place(file, "review"))?;
    let against = optional_str(mapping, "against")
        .map_err(|_| ConfigFailure::at(file, "review.against must be a non-empty target name"))?;
    let downstream = match get(mapping, "downstream") {
        None => None,
        Some(ConfigValue::String(text)) if REVIEW_DOWNSTREAM_POLICIES.contains(&text.as_str()) => {
            Some(text.clone())
        }
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                format!(
                    "review.downstream must be one of: {}",
                    REVIEW_DOWNSTREAM_POLICIES.join(", ")
                ),
            ));
        }
    };
    let valid_prefix = |prefix: &str| {
        prefix
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && prefix
                .chars()
                .all(|item| item.is_ascii_alphanumeric() || item == '_')
    };
    let schema_prefix: Option<String> = match get(mapping, "schema_prefix") {
        None => None,
        Some(ConfigValue::String(text)) if valid_prefix(text) => Some(text.clone()),
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "review.schema_prefix must start with a letter and contain only letters, digits \
                 and underscores",
            ));
        }
    };
    let critical_tags = match get(mapping, "critical_tags") {
        None => None,
        payload => Some(string_sequence(
            payload,
            place(file, "review.critical_tags"),
        )?),
    };
    Ok(ReviewConfig {
        against,
        downstream,
        schema_prefix,
        critical_tags,
    })
}

fn janitor_count(mapping: &Entries, key: &str, default: i64) -> Loaded<ConfigValue> {
    optional_int(mapping, key, default)
}

/// `_load_janitor`.
pub(crate) fn load_janitor(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "janitor"))?;
    reject_removed_virtual_keys(mapping, &["max_checkpoints"], place(file, "janitor"))?;
    let enabled = optional_bool(mapping, "enabled", false)?;
    let retention_days = optional_nullable_int(mapping, "retention_days")?;
    let archive = janitor_count(mapping, "archive_retention_days", 14)?;
    let history = janitor_count(mapping, "direct_state_history_versions", 20)?;
    let delete_tracked_only = optional_bool(mapping, "delete_tracked_only", true)?;
    let exclude = string_sequence(
        get(mapping, "exclude_patterns"),
        place(file, "janitor.exclude_patterns"),
    )?;
    let keep_in_use = optional_bool(mapping, "keep_in_use", false)?;
    let in_use_window = match get(mapping, "in_use_window") {
        None => DEFAULT_IN_USE_WINDOW.to_owned(),
        Some(ConfigValue::String(window)) if fixed_duration_seconds(window, false)?.is_some() => {
            window.clone()
        }
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "janitor.in_use_window must be a positive fixed duration such as '7d'",
            ));
        }
    };
    let checks = [
        (retention_days.as_ref(), "retention_days"),
        (Some(&archive), "archive_retention_days"),
        (Some(&history), "direct_state_history_versions"),
    ];
    if let Some((_, key)) = checks
        .iter()
        .find(|(value, _)| value.is_some_and(|number| int_below(number, 0)))
    {
        return Err(ConfigFailure::at(
            file,
            format!("janitor.{key} must be >= 0"),
        ));
    }
    Ok(object(
        "JanitorConfig",
        vec![
            ("enabled", Built::Value(ConfigValue::Bool(enabled))),
            ("retention_days", value_or_none(retention_days)),
            ("archive_retention_days", Built::Value(archive)),
            ("direct_state_history_versions", Built::Value(history)),
            (
                "delete_tracked_only",
                Built::Value(ConfigValue::Bool(delete_tracked_only)),
            ),
            ("exclude_patterns", Built::Tuple(exclude)),
            (
                "in_use_window",
                Built::Value(ConfigValue::String(in_use_window)),
            ),
            ("keep_in_use", Built::Value(ConfigValue::Bool(keep_in_use))),
        ],
    ))
}

/// `_load_snapshots`.
pub(crate) fn load_snapshots(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "snapshots"))?;
    validate_allowed_keys(mapping, &SNAPSHOT_KEYS, place(file, "snapshots"))?;
    let policies: [(&'static str, &[&str], &str); 4] = [
        (
            "current_state_full_refresh",
            &SNAPSHOT_FULL_REFRESH_POLICIES,
            "deny",
        ),
        (
            "historical_full_refresh",
            &SNAPSHOT_FULL_REFRESH_POLICIES,
            "require_confirmation",
        ),
        (
            "schema_change",
            &SNAPSHOT_SCHEMA_CHANGE_POLICIES,
            "append_new_columns",
        ),
        (
            "wildcard_check_schema_change",
            &SNAPSHOT_SCHEMA_CHANGE_POLICIES,
            "require_confirmation",
        ),
    ];
    let mut fields: Fields = Vec::new();
    for (key, choices, default) in policies {
        let policy = optional_choice(mapping, key, choices, default)?;
        fields.push((key, Built::Value(ConfigValue::String(policy))));
    }
    Ok(object("SnapshotsConfig", fields))
}

fn is_negative(value: Option<&ConfigValue>) -> bool {
    value.is_some_and(|number| int_below(number, 0))
}

fn is_run_namespace(value: &ConfigValue) -> bool {
    matches!(value, ConfigValue::String(text)
        if (1..=RUN_NAMESPACE_MAX_LENGTH).contains(&text.len())
            && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-')))
}

fn local_type_overrides(mapping: &Entries, file: ConfigFile) -> Loaded<Built> {
    let overrides = coerce_mapping(
        get(mapping, "local_type_overrides"),
        place(file, "scenario.local_type_overrides"),
    )?;
    let mut loaded: Vec<(String, Built)> = Vec::new();
    for (dialect, rules) in overrides {
        let name = str::trim(key_name(dialect));
        if name.is_empty() {
            return Err(ConfigFailure::at(
                file,
                "scenario.local_type_overrides contains an empty dialect key",
            ));
        }
        loaded = insert_entry(loaded, name.to_owned(), string_mapping(Some(rules), file)?);
    }
    Ok(Built::Dict(loaded))
}

fn snapshot_limits(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "scenario.snapshot_limits"))?;
    validate_allowed_keys(
        mapping,
        &SNAPSHOT_LIMIT_KEYS,
        place(file, "scenario.snapshot_limits"),
    )?;
    let limits = SNAPSHOT_LIMIT_KEYS
        .into_iter()
        .map(|key| keyed(key, optional_nullable_int(mapping, key)))
        .collect::<Loaded<Vec<_>>>()?;
    if limits.iter().any(|(_, value)| is_negative(value.as_ref())) {
        return Err(ConfigFailure::at(
            file,
            "scenario.snapshot_limits values must be >= 0",
        ));
    }
    Ok(object(
        "ScenarioSnapshotLimitsConfig",
        limits
            .into_iter()
            .map(|(key, value)| (key, value_or_none(value)))
            .collect(),
    ))
}

/// `_load_scenario`.
pub(crate) fn load_scenario(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "scenario"))?;
    validate_allowed_keys(mapping, &SCENARIO_KEYS, place(file, "scenario"))?;
    let overrides = local_type_overrides(mapping, file)?;
    let run_namespace = match get(mapping, RUN_NAMESPACE_KEY) {
        None => Built::Value(ConfigValue::Null),
        Some(value) if is_run_namespace(value) => Built::Value(value.clone()),
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "scenario.run_namespace must be 1–128 characters: ASCII letters, digits, '-', \
                 '_' or '.'",
            ));
        }
    };
    Ok(object(
        "ScenarioConfig",
        vec![
            ("run_namespace", run_namespace),
            ("local_type_overrides", overrides),
            (
                "snapshot_limits",
                snapshot_limits(get(mapping, "snapshot_limits"), file)?,
            ),
        ],
    ))
}

fn present_keys<'a>(mapping: &Entries, keys: &[&'a str]) -> Vec<&'a str> {
    keys.iter()
        .copied()
        .filter(|key| has(mapping, key))
        .collect()
}

/// `_load_dbt`.
pub(crate) fn load_dbt(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "dbt"))?;
    let reuse = present_keys(mapping, &DBT_REUSE_KEYS);
    if !reuse.is_empty() {
        return Err(ConfigFailure::at(
            file,
            format!(
                "[dbt] reuse option(s) were removed: {}; dbt production-ref reuse is no longer supported",
                reuse.join(", ")
            ),
        ));
    }
    let removed = present_keys(mapping, &DBT_REMOVED_KEYS);
    if !removed.is_empty() {
        return Err(ConfigFailure::at(
            file,
            format!(
                "[dbt] option(s) were removed: {}; use dbt-native --state/--defer",
                removed.join(", ")
            ),
        ));
    }
    validate_allowed_keys(mapping, &DBT_KEYS, place(file, "dbt"))?;
    let mut fields: Fields = DBT_KEYS[..4]
        .iter()
        .map(|key| keyed(*key, optional_text_field(mapping, key)))
        .collect::<Loaded<Fields>>()?;
    fields.push(("vars", object_mapping(get(mapping, "vars"), file)?));
    Ok(object("DbtConfig", fields))
}

/// `_load_local_dbt`.
pub(crate) fn load_local_dbt(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(payload, place(file, "dbt"))?;
    if has(mapping, DBT_DEFER_CLONE_KEY) {
        return Err(ConfigFailure::at(
            file,
            "[dbt] defer_clone_from was removed; remove it and use dbt-native --state/--defer",
        ));
    }
    validate_allowed_keys(mapping, &LOCAL_DBT_KEYS, place(file, "dbt"))?;
    Ok(object(
        "LocalDbtConfig",
        vec![
            ("target", text_or_none(optional_str(mapping, "target")?)),
            ("vars", object_mapping(get(mapping, "vars"), file)?),
        ],
    ))
}
