//! Contract, materialization, storage, migration and placeholder rules with Python's errors.

use crate::execution_limits::constants::MODEL_MAX_DURATION_KEY;
use crate::execution_limits::main::parse_execution_limit::parse_execution_limit;
use std::collections::{BTreeSet, HashMap};

use sqlbuild_core::text::main::is_word_char::is_word_char;

use crate::errors::ConfigError;
use crate::model_validation::_helpers::config::python_lower;
use crate::model_validation::_helpers::config::{
    ConfigView, model_header_help, non_blank_string_check, one_of, sorted_values, text_if_string,
};
use crate::model_validation::_helpers::durations::parse_duration;
use crate::model_validation::constants::{
    BUILTIN_MATERIALIZATIONS, CONTRACT_POLICIES, CUSTOM_MATERIALIZATION_DISALLOWED_KEYS,
    HISTORY_MATERIALIZATIONS, INCREMENTAL_MATERIALIZATION, INCREMENTAL_ONLY_KEYS,
    MICROBATCH_CONCURRENCY_HELP, MICROBATCH_CONCURRENCY_NOTE, OLD_NAME_VIEW_KEY, PLACEHOLDER_SIGIL,
    TABLE_BACKED_MATERIALIZATIONS, VIEW_MATERIALIZATION,
};
use crate::model_validation::constants::{
    CHAINED_HELP, CLAIMED_HELP, MATERIALIZATION_HELP, MIGRATABLE_MATERIALIZATIONS, ORIGIN_HELP,
    QUALIFIED_NAME_SEPARATOR,
};
use crate::model_validation::constants::{
    DYNAMIC_ADAPTER_CODE, DYNAMIC_ADAPTERS, DYNAMIC_MATERIALIZATION, DYNAMIC_ONLY_KEYS,
    DYNAMIC_REFRESH_FORMS, DYNAMIC_REFRESH_MODES, DYNAMIC_REFRESH_UNSUPPORTED_CODE,
    MATERIALIZED_VIEW_ADAPTER_CODE, MATERIALIZED_VIEW_MATERIALIZATION, SNAPSHOT_DISALLOWED_KEYS,
    SNOWFLAKE_ADAPTER, SNOWFLAKE_MIN_LAG_SECONDS,
};
use crate::model_validation::constants::{
    ENGINE_FAMILY_SUFFIX, ENGINE_KEY, ORDER_BY_KEY, PARTITION_BY_KEY, PHYSICAL_STORAGE_KEYS,
    PHYSICAL_STORAGE_MATERIALIZATIONS, PRIMARY_KEY_KEY, TABLE_SETTINGS_KEY,
    UNSUPPORTED_ENGINE_PREFIXES,
};
use crate::model_validation::models::{
    ModelValidationFacts, ProjectValidationFacts, ValidationStop,
};
use crate::model_validation::types::Check;
use crate::types::{AuthoredNode, NodeKind};

const MIGRATE_FROM_KEY: &str = "migrate_from";
/// The model key that marks a model whose downstream paths `sqb review` validates.
const CRITICAL_KEY: &str = "critical";
const MIGRATE_FORCE_KEY: &str = "migrate_force";
const REFRESH_KEY: &str = "refresh";
const LAG_FORM: &str = "lag";
const REFRESH_MODE_KEY: &str = "refresh_mode";
const REFRESH_WAREHOUSE_KEY: &str = "refresh_warehouse";
const REFRESH_ON_BUILD_KEY: &str = "refresh_on_build";

/// Check `validate_microbatch_project_capability`.
pub(crate) fn check_project_capability<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    project: &ProjectValidationFacts,
) -> Check {
    let Some(concurrency) = config.get("batch_concurrency") else {
        return Ok(());
    };
    let concurrent = match (concurrency.kind(), concurrency.integer()) {
        (NodeKind::Int { negative: false }, Some(value)) => value > 1,
        (NodeKind::Int { negative: false }, None) => true,
        _ => false,
    };
    if concurrent && !project.microbatch_concurrency {
        return Err(ValidationStop::Error(
            config
                .config_error(format!(
                    "batch_concurrency > 1 requires concurrent microbatches; \
                     {MICROBATCH_CONCURRENCY_NOTE}"
                ))
                .with_help(MICROBATCH_CONCURRENCY_HELP),
        ));
    }
    Ok(())
}

/// Check `validate_contract_config`.
pub(crate) fn check_contract<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(contract) = config.get("contract") else {
        return Ok(());
    };
    if contract.kind() != NodeKind::Str {
        return Err(config.error("contract must be a string"));
    }
    if CONTRACT_POLICIES
        .iter()
        .any(|policy| contract.is_text(policy))
    {
        return Ok(());
    }
    let contract: String = contract.text().unwrap_or_default();
    Err(config.error(format!(
        "unknown contract '{contract}'; valid values: {}",
        sorted_values(&CONTRACT_POLICIES)
    )))
}

/// Check that `max_duration` is a positive fixed duration or `disabled`.
pub(crate) fn check_execution_limit<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(text) = config.string(MODEL_MAX_DURATION_KEY)? else {
        return Ok(());
    };
    if parse_execution_limit(&text).is_some() {
        return Ok(());
    }
    Err(config.error(format!(
        "{MODEL_MAX_DURATION_KEY} must be a positive fixed duration such as '15m' or '1h', or \
         'disabled', got '{text}'"
    )))
}

/// Check that the `critical` review marker is a boolean.
pub(crate) fn check_review_marker<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    match config.get(CRITICAL_KEY).map(AuthoredNode::kind) {
        None | Some(NodeKind::Bool(_)) => Ok(()),
        Some(_) => Err(config.error(format!("{CRITICAL_KEY} must be a boolean"))),
    }
}

/// Check `validate_non_incremental_config`.
pub(crate) fn check_non_incremental<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    if config.string("materialized")?.as_deref() == Some(INCREMENTAL_MATERIALIZATION) {
        return Ok(());
    }
    match config.first_present(&INCREMENTAL_ONLY_KEYS) {
        Some(key) => Err(config.error(format!("{key} is only valid for incremental models"))),
        None => Ok(()),
    }
}

/// Check `validate_custom_materialization_config`.
pub(crate) fn check_custom_materialization<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    project: &ProjectValidationFacts,
) -> Check {
    let Some(materialized) = config.string("materialized")? else {
        return Ok(());
    };
    if one_of(&materialized, &BUILTIN_MATERIALIZATIONS) {
        return Ok(());
    }
    if !project.custom_materializations.contains(&materialized) {
        return Err(config.error(format!(
            "unknown materialization '{materialized}'; not a built-in type and no custom \
             materialization with that name was discovered"
        )));
    }
    match config.first_present(&CUSTOM_MATERIALIZATION_DISALLOWED_KEYS) {
        Some(key) => Err(config.error(format!("{key} is not allowed on custom materializations"))),
        None => Ok(()),
    }
}

/// Check `validate_storage_policies`.
pub(crate) fn check_storage_policies<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    facts: &ModelValidationFacts<'_>,
) -> Check {
    let materialized = config.string("materialized")?;
    let materialized = materialized.as_deref();
    let shown = materialized.unwrap_or("None");
    let table_backed =
        materialized.is_some_and(|value| one_of(value, &TABLE_BACKED_MATERIALIZATIONS));
    if !facts.retention_unmanaged {
        if materialized == Some(VIEW_MATERIALIZATION) {
            return Err(config.error(
                "managed time_travel_retention is not valid for views; set \
                 time_travel_retention disabled",
            ));
        }
        if !table_backed {
            return Err(config.error(format!(
                "managed time_travel_retention is not supported for materialization '{shown}'"
            )));
        }
    }
    if facts.table_type_declared {
        if materialized == Some(VIEW_MATERIALIZATION) {
            return Err(config.error("table_type is not valid for views"));
        }
        if !table_backed {
            return Err(config.error(format!(
                "table_type is not supported for materialization '{shown}'"
            )));
        }
    }
    Ok(())
}

/// Check `validate_model_migration_config`.
pub(crate) fn check_migration<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    facts: &ModelValidationFacts<'_>,
) -> Check {
    check_old_name_view(config)?;
    let migrate_from = config.get(MIGRATE_FROM_KEY);
    let migrate_force = config.get(MIGRATE_FORCE_KEY);
    if migrate_from.is_none() && migrate_force.is_none() {
        return Ok(());
    }
    let materialized = config.string("materialized")?;
    let migratable = materialized.as_deref().is_some_and(|value| {
        value == VIEW_MATERIALIZATION || one_of(value, &TABLE_BACKED_MATERIALIZATIONS)
    });
    if !migratable {
        return Err(config
            .error("migrate_from is only valid for table, view, incremental and snapshot models"));
    }
    let materialized = materialized.unwrap_or_default();
    if migrate_force.is_some() && !one_of(&materialized, &HISTORY_MATERIALIZATIONS) {
        return Err(ValidationStop::Error(
            config
                .config_error(format!(
                    "migrate_force is only valid for incremental and snapshot models; nothing is \
                     replaced when a '{materialized}' model migrates, because tables and views \
                     are rebuilt under their new name"
                ))
                .with_help("remove migrate_force from the model header"),
        ));
    }
    let Some(migrate_from) = migrate_from else {
        return Err(config.error("migrate_force requires migrate_from"));
    };
    let origin: String = text_if_string(migrate_from)
        .map(|text| str::trim(&text).to_owned())
        .unwrap_or_default();
    if origin.is_empty() {
        return Err(config.error("migrate_from must be a model name or a qualified relation"));
    }
    if python_lower(&origin) == python_lower(facts.model_name) {
        return Err(config.error("migrate_from cannot name the model itself"));
    }
    match migrate_force.map(AuthoredNode::kind) {
        None | Some(NodeKind::Bool(_)) => Ok(()),
        Some(_) => Err(config.error("migrate_force must be true or false")),
    }
}

fn check_old_name_view<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(value) = config.raw(OLD_NAME_VIEW_KEY) else {
        return Ok(());
    };
    if value.kind() == NodeKind::Bool(false) {
        return Ok(());
    }
    if let Some(text) = text_if_string(value)
        && config
            .duration(OLD_NAME_VIEW_KEY, str::trim(&text))?
            .is_some()
    {
        return Ok(());
    }
    let shown: String = value.python_repr();
    Err(ValidationStop::Error(
        config
            .config_error(format!(
                "old_name_view must be a positive duration such as 7d, or false; got {shown}"
            ))
            .with_help("write for example old_name_view 7d, or old_name_view false"),
    ))
}

/// Check `validate_placeholder_config`.
pub(crate) fn check_placeholders<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    project: &ProjectValidationFacts,
    facts: &ModelValidationFacts<'_>,
) -> Check {
    let materialized = config.string("materialized")?;
    let custom = materialized
        .as_ref()
        .is_some_and(|value| project.custom_materializations.contains(value));
    let used = placeholder_names(facts.query_sql);
    let declared = config.get("placeholders");
    if !custom && !used.is_empty() {
        return Err(config.error("@@@placeholders are only allowed on custom materializations"));
    }
    if !custom && declared.is_some() {
        return Err(config.error("placeholders config is only allowed on custom materializations"));
    }
    if !custom || (used.is_empty() && declared.is_none()) {
        return Ok(());
    }
    let declared_names = match declared {
        Some(value) if value.kind() == NodeKind::Map => value
            .entries()
            .iter()
            .map(|(key, _)| key.python_str())
            .collect::<BTreeSet<String>>(),
        _ => BTreeSet::new(),
    };
    let missing: Vec<&str> = used
        .difference(&declared_names)
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(config.error(format!(
            "@@@placeholders without default values in placeholders config: {}",
            missing.join(", ")
        )));
    }
    let unused: Vec<&str> = declared_names
        .difference(&used)
        .map(String::as_str)
        .collect();
    if !unused.is_empty() {
        return Err(config.error(format!(
            "placeholders config entries not used in SQL: {}",
            unused.join(", ")
        )));
    }
    Ok(())
}

/// Return the names `re.findall(r"@@@(\w+)", sql)` finds; `\w` is Python's word class.
fn placeholder_names(sql: &str) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let mut rest = sql;
    while let Some(start) = rest.find(PLACEHOLDER_SIGIL) {
        let after = &rest[start + PLACEHOLDER_SIGIL.len()..];
        let length: usize = after
            .char_indices()
            .find(|(_, character)| !is_word_char(*character))
            .map_or(after.len(), |(at, _)| at);
        if length == 0 {
            rest = &rest[start + 1..];
            continue;
        }
        names.insert(after[..length].to_owned());
        rest = &after[length..];
    }
    names
}

/// Column renames need a history model, or a table or view that migrates itself.
pub(crate) fn column_materialization_problem(
    model_name: &str,
    materialized: Option<&str>,
    model_migrates: bool,
    first: &str,
) -> Result<(), ConfigError> {
    let history: bool = materialized.is_some_and(|value| HISTORY_MATERIALIZATIONS.contains(&value));
    let migratable: bool =
        materialized.is_some_and(|value| MIGRATABLE_MATERIALIZATIONS.contains(&value));
    if !history && (!migratable || !model_migrates) {
        let shown: &str = materialized.unwrap_or("None");
        return Err(ConfigError::compile(format!(
            "model '{model_name}': column '{first}' declares migrate_from, which is only valid \
             for incremental and snapshot models, or for table and view models that declare \
             migrate_from themselves (the old name's compatibility view then presents the column \
             under its old name); '{shown}' models are rebuilt with their new columns"
        ))
        .with_help(MATERIALIZATION_HELP));
    }
    Ok(())
}

/// Each origin names one other column, claimed by only one rename.
pub(crate) fn origin_problem(
    model_name: &str,
    declared: &[(&str, &str)],
) -> Result<(), ConfigError> {
    let mut sources: HashMap<String, &str> = HashMap::new();
    for &(name, origin) in declared {
        let origin: &str = str::trim(origin);
        if origin.is_empty() || origin.contains(QUALIFIED_NAME_SEPARATOR) {
            return Err(ConfigError::compile(format!(
                "model '{model_name}': column '{name}' migrate_from must name one column of the \
                 model's existing table"
            ))
            .with_help(ORIGIN_HELP));
        }
        if python_lower(origin) == python_lower(name) {
            return Err(ConfigError::compile(format!(
                "model '{model_name}': column '{name}' migrate_from cannot name the column itself"
            )));
        }
        if let Some(claimed_by) = sources.get(&python_lower(origin)) {
            return Err(ConfigError::compile(format!(
                "model '{model_name}': columns '{claimed_by}' and '{name}' both declare \
                 migrate_from {origin}; a column can be renamed to only one new name"
            ))
            .with_help(CLAIMED_HELP));
        }
        sources.insert(python_lower(origin), name);
    }
    Ok(())
}

/// No rename may start from another renamed column.
pub(crate) fn chained_problem(
    model_name: &str,
    declared: &[(&str, &str)],
) -> Result<(), ConfigError> {
    let destinations: HashMap<String, &str> = declared
        .iter()
        .map(|&(name, _)| (python_lower(name), name))
        .collect();
    for &(name, origin) in declared {
        if destinations.contains_key(&python_lower(str::trim(origin))) {
            return Err(ConfigError::compile(format!(
                "model '{model_name}': column '{name}' migrates from '{origin}', which itself \
                 declares migrate_from; chained or swapped column renames are not supported"
            ))
            .with_help(CHAINED_HELP));
        }
    }
    Ok(())
}

/// Check the physical layout keys a table-backed model may set.
pub(crate) fn check_physical_storage<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(first) = config.first_present(&PHYSICAL_STORAGE_KEYS) else {
        return Ok(());
    };
    let materialized: Option<String> = config.string("materialized")?;
    if !materialized
        .as_deref()
        .is_some_and(|value| one_of(value, &PHYSICAL_STORAGE_MATERIALIZATIONS))
    {
        return Err(config.error(format!(
            "{first} is only valid for table and incremental models"
        )));
    }
    check_engine(config)?;
    let order_by: Option<Vec<String>> = expressions(config, ORDER_BY_KEY)?;
    if let Some(value) = config.get(PARTITION_BY_KEY)
        && !non_blank_string_check(value)
    {
        return Err(config.error(format!(
            "{PARTITION_BY_KEY} must be a non-empty SQL expression string"
        )));
    }
    check_primary_key(config, order_by.as_deref())?;
    check_table_settings(config)
}

fn check_engine<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(value) = config.get(ENGINE_KEY) else {
        return Ok(());
    };
    let engine: Option<String> = text_if_string(value).map(|text| text.trim().to_owned());
    let name: Option<&str> = engine
        .as_deref()
        .and_then(|engine| engine_name(engine))
        .filter(|name| name.ends_with(ENGINE_FAMILY_SUFFIX));
    match name {
        Some(name)
            if !UNSUPPORTED_ENGINE_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix)) =>
        {
            Ok(())
        }
        _ => Err(ValidationStop::Error(
            config
                .config_error(format!(
                    "{ENGINE_KEY} '{}' must be a non-replicated MergeTree-family table engine",
                    value.python_str()
                ))
                .with_help(model_header_help(
                    "to name a MergeTree-family engine",
                    "engine 'ReplacingMergeTree(updated_at)'",
                )),
        )),
    }
}

/// The engine name before its parenthesized arguments, when the text is shaped like one.
fn engine_name(engine: &str) -> Option<&str> {
    let (name, rest) = match engine.find('(') {
        Some(open) => (&engine[..open], &engine[open..]),
        None => (engine, ""),
    };
    let named: bool = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    let arguments: bool = rest.is_empty() || rest.ends_with(')');
    (named && arguments).then_some(name)
}

/// The expressions of a string or a list of strings, failing for anything else.
fn expressions<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    key: &str,
) -> Result<Option<Vec<String>>, ValidationStop> {
    let Some(value) = config.get(key) else {
        return Ok(None);
    };
    let items: Vec<N> = match value.kind() {
        NodeKind::Str => vec![value.clone()],
        NodeKind::List | NodeKind::Tuple => value.items(),
        _ => Vec::new(),
    };
    let texts: Vec<String> = items
        .iter()
        .filter(|item| non_blank_string_check(*item))
        .filter_map(text_if_string)
        .map(|text| text.trim().to_owned())
        .collect();
    if texts.is_empty() || texts.len() != items.len() {
        return Err(config.error(format!(
            "{key} must be a SQL expression string or a list of them"
        )));
    }
    Ok(Some(texts))
}

fn check_primary_key<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    order_by: Option<&[String]>,
) -> Check {
    let Some(primary_key) = expressions(config, PRIMARY_KEY_KEY)? else {
        return Ok(());
    };
    let primary_key: Vec<String> = key_elements(&primary_key);
    let prefix: bool =
        order_by.is_some_and(|order_by| key_elements(order_by).starts_with(&primary_key));
    if prefix {
        return Ok(());
    }
    Err(config.error(format!(
        "{PRIMARY_KEY_KEY} must be a prefix of {ORDER_BY_KEY}"
    )))
}

/// The key's element expressions: each text unwrapped and split at its top-level commas.
fn key_elements(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .flat_map(|text| top_level_items(unwrapped(text.trim())))
        .collect()
}

/// `text` without one pair of parentheses enclosing all of it.
fn unwrapped(text: &str) -> &str {
    let inner: Option<&str> = text
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'));
    match inner {
        Some(inner) if balanced(inner) => inner,
        _ => text,
    }
}

/// Whether every bracket in `text` outside quotes closes within it.
fn balanced(text: &str) -> bool {
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    for character in text.chars() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(character),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            (None, _) => {}
        }
    }
    depth == 0 && quote.is_none()
}

/// `text` split at commas outside quotes and brackets, each item trimmed.
fn top_level_items(text: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    let mut current: String = String::new();
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    for character in text.chars() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(character),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth -= 1,
            (None, ',') if depth == 0 => {
                items.push(current.trim().to_owned());
                current.clear();
                continue;
            }
            (None, _) => {}
        }
        current.push(character);
    }
    items.push(current.trim().to_owned());
    items
}

fn check_table_settings<N: AuthoredNode>(config: &ConfigView<'_, N>) -> Check {
    let Some(value) = config.get(TABLE_SETTINGS_KEY) else {
        return Ok(());
    };
    let valid: bool = value.kind() == NodeKind::Map
        && value.entries().iter().all(|(key, setting)| {
            key.with_text(setting_name).unwrap_or(false)
                && matches!(
                    setting.kind(),
                    NodeKind::Str | NodeKind::Bool(_) | NodeKind::Int { .. }
                )
        });
    if valid {
        return Ok(());
    }
    Err(config.error(format!(
        "{TABLE_SETTINGS_KEY} must map setting names to strings, integers or booleans"
    )))
}

fn setting_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// Check the refresh fields: only dynamic models on an adapter that builds them take them.
pub(crate) fn check_dynamic<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    project: &ProjectValidationFacts,
) -> Check {
    let materialized: Option<String> = config.string("materialized")?;
    let adapter: Option<&str> = project.adapter.as_deref();
    if materialized.as_deref() == Some(MATERIALIZED_VIEW_MATERIALIZATION)
        && adapter == Some(SNOWFLAKE_ADAPTER)
        && !project
            .custom_materializations
            .contains(MATERIALIZED_VIEW_MATERIALIZATION)
    {
        return Err(coded(
            config,
            MATERIALIZED_VIEW_ADAPTER_CODE,
            "Snowflake has no materialized_view materialization; use materialized dynamic, \
             which builds a dynamic table",
        ));
    }
    if materialized.as_deref() != Some(DYNAMIC_MATERIALIZATION) {
        return match config.first_present(&DYNAMIC_ONLY_KEYS) {
            Some(key) => Err(config.error(format!("{key} is only valid for dynamic models"))),
            None => Ok(()),
        };
    }
    if let Some(adapter) = adapter.filter(|adapter| !one_of(adapter, &DYNAMIC_ADAPTERS)) {
        return Err(coded(
            config,
            DYNAMIC_ADAPTER_CODE,
            format!(
                "materialized dynamic is not supported on the {adapter} adapter; adapters \
                 that support it: {}",
                DYNAMIC_ADAPTERS.join(", ")
            ),
        ));
    }
    if let Some(key) = config.first_present(&SNAPSHOT_DISALLOWED_KEYS) {
        return Err(config.error(format!(
            "{key} is not valid for dynamic models: the warehouse refreshes them from the \
             whole query"
        )));
    }
    check_refresh(config, adapter)?;
    if let Some(mode) = config.get(REFRESH_MODE_KEY) {
        let text: Option<String> = text_if_string(mode);
        if !text
            .as_deref()
            .is_some_and(|text| one_of(text, &DYNAMIC_REFRESH_MODES))
        {
            return Err(config.error(format!(
                "{REFRESH_MODE_KEY} must be one of: {}; got '{}'",
                sorted_values(&DYNAMIC_REFRESH_MODES),
                text.unwrap_or_default()
            )));
        }
    }
    if config
        .get(REFRESH_WAREHOUSE_KEY)
        .is_some_and(|warehouse| !non_blank_string_check(warehouse))
    {
        return Err(config.error(format!(
            "{REFRESH_WAREHOUSE_KEY} must be a non-empty warehouse name"
        )));
    }
    match config.get(REFRESH_ON_BUILD_KEY).map(AuthoredNode::kind) {
        None | Some(NodeKind::Bool(_)) => Ok(()),
        Some(_) => Err(config.error(format!("{REFRESH_ON_BUILD_KEY} must be a boolean"))),
    }
}

/// Check that `refresh` is a block with exactly one known form and a valid value.
fn check_refresh<N: AuthoredNode>(config: &ConfigView<'_, N>, adapter: Option<&str>) -> Check {
    let Some(refresh) = config.get(REFRESH_KEY) else {
        return Ok(());
    };
    if refresh.kind() != NodeKind::Map {
        return Err(config.error(format!(
            "{REFRESH_KEY} must be a block such as refresh (lag 1h)"
        )));
    }
    let block: Vec<(N, N)> = refresh.entries();
    let forms: Vec<String> = block
        .iter()
        .map(|(name, _)| name.text().unwrap_or_default())
        .collect();
    let [(form, value)] = block.as_slice() else {
        return Err(unknown_form(config, &forms));
    };
    if !DYNAMIC_REFRESH_FORMS
        .iter()
        .any(|known| form.is_text(known))
    {
        return Err(unknown_form(config, &forms));
    }
    let text: String = text_if_string(value).unwrap_or_default();
    let Ok(Some(lag)) = parse_duration(&text) else {
        return Err(not_fixed(config, &text));
    };
    if lag.total_months != 0 {
        return Err(not_fixed(config, &text));
    }
    if adapter == Some(SNOWFLAKE_ADAPTER) && lag.fixed_seconds < SNOWFLAKE_MIN_LAG_SECONDS {
        return Err(coded(
            config,
            DYNAMIC_REFRESH_UNSUPPORTED_CODE,
            format!("{REFRESH_KEY} {LAG_FORM} on Snowflake must be at least 1m; got '{text}'"),
        ));
    }
    Ok(())
}

fn not_fixed<N: AuthoredNode>(config: &ConfigView<'_, N>, text: &str) -> ValidationStop {
    config.error(format!(
        "{REFRESH_KEY} {LAG_FORM} must be a fixed duration such as 5m, 1h or 1d; got '{text}'"
    ))
}

fn coded<N: AuthoredNode>(
    config: &ConfigView<'_, N>,
    code: &'static str,
    text: impl std::fmt::Display,
) -> ValidationStop {
    ValidationStop::Error(config.config_error(text).with_code(code))
}

fn unknown_form<N: AuthoredNode>(config: &ConfigView<'_, N>, forms: &[String]) -> ValidationStop {
    config.error(format!(
        "{REFRESH_KEY} takes exactly one form, such as refresh (lag 1h); got: {}",
        forms.join(", ")
    ))
}
