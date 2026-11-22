//! Typed reads of configuration fields with the Python loader's defaults and type checks.

use crate::models::ConfigValue;
use crate::project::_helpers::keys::Place;
use crate::project::_helpers::values::{int_below, is_int};
use crate::project::errors::ConfigFailure;
use crate::project::models::{Built, ConfigFile};
use crate::project::types::{Entries, Loaded};

/// `mapping.get(key)`.
pub(crate) fn get<'a>(mapping: &'a Entries, key: &str) -> Option<&'a ConfigValue> {
    mapping
        .iter()
        .find(|(candidate, _)| matches!(candidate, ConfigValue::String(text) if text == key))
        .map(|(_, value)| value)
        .filter(|value| !matches!(value, ConfigValue::Null))
}

/// `key in mapping`.
pub(crate) fn has(mapping: &Entries, key: &str) -> bool {
    get(mapping, key).is_some()
}

/// The text of a string key.
pub(crate) fn key_name(key: &ConfigValue) -> &str {
    match key {
        ConfigValue::String(text) => text,
        _ => "",
    }
}

/// `_coerce_mapping`: a missing value is empty; anything else must be a mapping.
pub(crate) fn coerce_mapping<'a>(
    payload: Option<&'a ConfigValue>,
    place: Place<'_>,
) -> Loaded<&'a Entries> {
    match payload {
        None => Ok(&[]),
        Some(ConfigValue::Map(entries)) => Ok(entries),
        Some(_) => Err(ConfigFailure::at(
            place.file,
            format!("{} must be a mapping", place.label),
        )),
    }
}

fn non_empty(value: &ConfigValue) -> Option<String> {
    match value {
        ConfigValue::String(text) if !str::trim(text).is_empty() => {
            Some(str::trim(text).to_owned())
        }
        _ => None,
    }
}

/// `_optional_str`: a stripped non-empty string, or `None` when absent.
pub(crate) fn optional_str(mapping: &Entries, key: &str) -> Loaded<Option<String>> {
    let Some(value) = get(mapping, key) else {
        return Ok(None);
    };
    non_empty(value).map(Some).ok_or_else(|| {
        ConfigFailure::plain(format!(
            "Expected '{key}' to be a non-empty string when provided"
        ))
    })
}

/// A string that may be empty, such as `env_file = ""`, as a field value; `None` when unset.
pub(crate) fn optional_text_allowing_empty(mapping: &Entries, key: &str) -> Loaded<Built> {
    match get(mapping, key) {
        None => Ok(text_or_none(None)),
        Some(ConfigValue::String(text)) => Ok(text_or_none(Some(text.clone()))),
        Some(_) => Err(ConfigFailure::plain(format!(
            "Expected '{key}' to be a string when provided"
        ))),
    }
}

/// `_optional_str` as a field value.
pub(crate) fn optional_text_field(mapping: &Entries, key: &str) -> Loaded<Built> {
    Ok(text_or_none(optional_str(mapping, key)?))
}

/// A string, or `None`.
pub(crate) fn text_or_none(value: Option<String>) -> Built {
    Built::Value(value.map_or(ConfigValue::Null, ConfigValue::String))
}

/// `_require_str`.
pub(crate) fn require_str(mapping: &Entries, key: &str, file: ConfigFile) -> Loaded<String> {
    get(mapping, key)
        .and_then(non_empty)
        .ok_or_else(|| ConfigFailure::at(file, format!("must define non-empty string '{key}'")))
}

/// `_optional_mapping`: the authored mapping, or an empty one.
pub(crate) fn optional_mapping(mapping: &Entries, key: &str) -> Loaded<ConfigValue> {
    match get(mapping, key) {
        None => Ok(ConfigValue::Map(Vec::new())),
        Some(value @ ConfigValue::Map(_)) => Ok(value.clone()),
        Some(_) => Err(ConfigFailure::plain(format!(
            "Expected '{key}' to be a mapping when provided"
        ))),
    }
}

fn expected(key: &str, kind: &str) -> ConfigFailure {
    ConfigFailure::plain(format!("Expected '{key}' to be {kind} when provided"))
}

/// `_optional_nullable_bool`.
pub(crate) fn optional_nullable_bool(mapping: &Entries, key: &str) -> Loaded<Option<bool>> {
    match get(mapping, key) {
        None => Ok(None),
        Some(ConfigValue::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(expected(key, "a boolean")),
    }
}

/// `_optional_bool`.
pub(crate) fn optional_bool(mapping: &Entries, key: &str, default: bool) -> Loaded<bool> {
    Ok(optional_nullable_bool(mapping, key)?.unwrap_or(default))
}

/// A boolean, or `None`.
pub(crate) fn bool_or_none(value: Option<bool>) -> Built {
    Built::Value(value.map_or(ConfigValue::Null, ConfigValue::Bool))
}

/// `_optional_templated_bool`: a boolean or a template string.
pub(crate) fn optional_templated_bool(mapping: &Entries, key: &str) -> Loaded<Built> {
    match get(mapping, key) {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(value @ (ConfigValue::Bool(_) | ConfigValue::String(_))) => {
            Ok(Built::Value(value.clone()))
        }
        Some(_) => Err(expected(key, "a boolean")),
    }
}

/// `_optional_nullable_int`: any Python integer, booleans included.
pub(crate) fn optional_nullable_int(mapping: &Entries, key: &str) -> Loaded<Option<ConfigValue>> {
    match get(mapping, key) {
        None => Ok(None),
        Some(value) if is_int(value) => Ok(Some(value.clone())),
        Some(_) => Err(expected(key, "an integer")),
    }
}

/// `_optional_int`.
pub(crate) fn optional_int(mapping: &Entries, key: &str, default: i64) -> Loaded<ConfigValue> {
    Ok(optional_nullable_int(mapping, key)?.unwrap_or(ConfigValue::Integer(default)))
}

/// `_optional_strict_int`: an integer that is not a boolean.
pub(crate) fn optional_strict_int(mapping: &Entries, key: &str) -> Loaded<Option<ConfigValue>> {
    if matches!(get(mapping, key), Some(ConfigValue::Bool(_))) {
        return Err(expected(key, "an integer"));
    }
    optional_nullable_int(mapping, key)
}

/// `_optional_non_negative_int`.
pub(crate) fn optional_non_negative_int(
    mapping: &Entries,
    key: &str,
) -> Loaded<Option<ConfigValue>> {
    let value = optional_strict_int(mapping, key)?;
    if value.as_ref().is_some_and(|number| int_below(number, 0)) {
        return Err(ConfigFailure::plain(format!(
            "Expected '{key}' to be zero or greater when provided"
        )));
    }
    Ok(value)
}

/// An optional value as a field.
pub(crate) fn value_or_none(value: Option<ConfigValue>) -> Built {
    Built::Value(value.unwrap_or(ConfigValue::Null))
}

/// `_optional_scalar_batch_size`: a string or an integer.
pub(crate) fn optional_batch_size(mapping: &Entries, key: &str) -> Loaded<Built> {
    match get(mapping, key) {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(value @ ConfigValue::String(_)) => Ok(Built::Value(value.clone())),
        Some(value) if is_int(value) => Ok(Built::Value(value.clone())),
        Some(_) => Err(expected(key, "a string or integer")),
    }
}

/// `_optional_cursor_start`: a string, integer, date or datetime.
pub(crate) fn optional_cursor_start(mapping: &Entries, key: &str) -> Loaded<Built> {
    match get(mapping, key) {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(
            value @ (ConfigValue::String(_) | ConfigValue::Date(_) | ConfigValue::DateTime(_)),
        ) => Ok(Built::Value(value.clone())),
        Some(value) if is_int(value) => Ok(Built::Value(value.clone())),
        Some(_) => Err(expected(key, "a string, integer, or date-like value")),
    }
}

/// A string value from a fixed vocabulary, or `default` when absent.
pub(crate) fn optional_choice(
    mapping: &Entries,
    key: &str,
    choices: &[&str],
    default: &str,
) -> Loaded<String> {
    match get(mapping, key) {
        None => Ok(default.to_owned()),
        Some(ConfigValue::String(text)) if choices.contains(&text.as_str()) => Ok(text.clone()),
        Some(ConfigValue::String(_)) => Err(ConfigFailure::plain(format!(
            "Expected '{key}' to be one of: {}",
            choices.join(", ")
        ))),
        Some(_) => Err(expected(key, "a string")),
    }
}

/// `_optional_contract_policy`.
pub(crate) fn optional_contract_policy(
    mapping: &Entries,
    key: &str,
    policies: &[&str],
) -> Loaded<Built> {
    match get(mapping, key) {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(value @ ConfigValue::String(text)) if policies.contains(&text.as_str()) => {
            Ok(Built::Value(value.clone()))
        }
        Some(ConfigValue::String(_)) => Err(ConfigFailure::plain(
            "Expected 'contract' to be one of: enforced, none",
        )),
        Some(_) => Err(expected(key, "a string")),
    }
}

/// `_load_string_sequence`: a list of strings, empty when absent.
pub(crate) fn string_sequence(
    payload: Option<&ConfigValue>,
    place: Place<'_>,
) -> Loaded<Vec<String>> {
    let items = match payload {
        None => return Ok(Vec::new()),
        Some(ConfigValue::List(items)) => items,
        Some(_) => {
            return Err(ConfigFailure::at(
                place.file,
                format!("{} must be a list of strings", place.label),
            ));
        }
    };
    items
        .iter()
        .map(|item| match item {
            ConfigValue::String(text) => Ok(text.clone()),
            _ => Err(ConfigFailure::at(
                place.file,
                format!("{} must contain only strings", place.label),
            )),
        })
        .collect()
}

/// `_load_string_mapping`: string values by key.
pub(crate) fn string_mapping(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(
        payload,
        Place {
            file,
            label: "mapping",
        },
    )?;
    mapping
        .iter()
        .map(|(key, value)| match value {
            ConfigValue::String(_) => Ok((key_name(key).to_owned(), Built::Value(value.clone()))),
            _ => Err(ConfigFailure::at(
                file,
                format!("expected string value for '{}'", key_name(key)),
            )),
        })
        .collect::<Loaded<Vec<_>>>()
        .map(Built::Dict)
}

/// `_load_object_mapping`: the authored mapping, or an empty one.
pub(crate) fn object_mapping(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = coerce_mapping(
        payload,
        Place {
            file,
            label: "mapping",
        },
    )?;
    Ok(Built::Value(ConfigValue::Map(mapping.to_vec())))
}

/// `entries[key] = value`: a repeated key keeps its first position and takes the new value.
pub(crate) fn insert_entry(
    entries: Vec<(String, Built)>,
    key: String,
    value: Built,
) -> Vec<(String, Built)> {
    let mut updated = entries;
    match updated.iter_mut().find(|(existing, _)| *existing == key) {
        Some(entry) => entry.1 = value,
        None => updated.push((key, value)),
    }
    updated
}

/// A dataclass instance.
pub(crate) fn object(class: &'static str, fields: Vec<(&'static str, Built)>) -> Built {
    Built::Object(class, fields)
}

/// A `StrEnum` member by value.
pub(crate) fn member(class: &'static str, value: &str) -> Built {
    Built::Enum(class, value.to_owned())
}

/// A loaded value paired with its key.
pub(crate) fn keyed<K, T>(key: K, loaded: Loaded<T>) -> Loaded<(K, T)> {
    loaded.map(|value| (key, value))
}
