//! Loader for `[event_sinks]` and its lifecycle event sinks.

use crate::models::ConfigValue;
use crate::project::_helpers::fields::{get, key_name, object};
use crate::project::_helpers::keys::{Place, validate_allowed_keys};
use crate::project::_helpers::sections::fixed_duration_seconds;
use crate::project::_helpers::targets::seconds_value;
use crate::project::_helpers::values::compare_naturals;
use crate::project::constants::{
    EVENT_EXPORT_KINDS, EVENT_EXPORT_SEVERITIES, EVENT_FILTER_KEYS, EVENT_SINK_KEYS,
    SHUTDOWN_TIMEOUT_KEY, SHUTDOWN_TIMEOUT_MAX_SECONDS,
};
use crate::project::errors::ConfigFailure;
use crate::project::models::{Built, ConfigFile};
use crate::project::types::{Entries, Loaded};

fn event_kinds(mapping: &Entries, place: Place<'_>) -> Loaded<Built> {
    let Some(value) = get(mapping, "event_kinds") else {
        return Ok(Built::Value(ConfigValue::Null));
    };
    let invalid = || {
        ConfigFailure::at(
            place.file,
            format!("{}.event_kinds must be a list of strings", place.label),
        )
    };
    let ConfigValue::List(items) = value else {
        return Err(invalid());
    };
    let mut kinds: Vec<String> = Vec::new();
    for item in items {
        let ConfigValue::String(kind) = item else {
            return Err(invalid());
        };
        if !kinds.contains(kind) {
            kinds.push(kind.clone());
        }
    }
    let mut unknown: Vec<&str> = kinds
        .iter()
        .map(String::as_str)
        .filter(|kind| !EVENT_EXPORT_KINDS.contains(kind))
        .collect();
    unknown.sort_unstable();
    if !unknown.is_empty() {
        return Err(ConfigFailure::at(
            place.file,
            format!(
                "{}.event_kinds contains unknown kind(s): {}",
                place.label,
                unknown.join(", ")
            ),
        ));
    }
    Ok(Built::FrozenSet(kinds))
}

fn min_severity(mapping: &Entries, place: Place<'_>) -> Loaded<Built> {
    match get(mapping, "min_severity") {
        None => Ok(Built::Value(ConfigValue::Null)),
        Some(ConfigValue::String(text)) if EVENT_EXPORT_SEVERITIES.contains(&text.as_str()) => {
            Ok(Built::Enum("EventExportSeverity", text.clone()))
        }
        Some(_) => Err(ConfigFailure::at(
            place.file,
            format!(
                "{}.min_severity must be one of: {}",
                place.label,
                EVENT_EXPORT_SEVERITIES.join(", ")
            ),
        )),
    }
}

fn load_filter(mapping: &Entries, place: Place<'_>) -> Loaded<Built> {
    validate_allowed_keys(mapping, &EVENT_FILTER_KEYS, place)?;
    Ok(object(
        "LifecycleEventSinkFilterConfig",
        vec![
            ("event_kinds", event_kinds(mapping, place)?),
            ("min_severity", min_severity(mapping, place)?),
        ],
    ))
}

fn named_filters(config: &Entries, file: ConfigFile) -> Loaded<Built> {
    let named = match get(config, "named") {
        None => &[][..],
        Some(ConfigValue::Map(entries)) => entries.as_slice(),
        Some(_) => {
            return Err(ConfigFailure::at(
                file,
                "event_sinks.lifecycle.named must be a mapping",
            ));
        }
    };
    let mut loaded: Vec<(String, Built)> = Vec::new();
    for (key, value) in named {
        let name = key_name(key);
        if name.is_empty() {
            return Err(ConfigFailure::at(
                file,
                "event_sinks.lifecycle.named contains an invalid name",
            ));
        }
        let label = format!("event_sinks.lifecycle.named.{name}");
        let ConfigValue::Map(entries) = value else {
            return Err(ConfigFailure::at(
                file,
                format!("{label} must be a mapping"),
            ));
        };
        loaded.push((
            name.to_owned(),
            load_filter(
                entries,
                Place {
                    file,
                    label: &label,
                },
            )?,
        ));
    }
    Ok(Built::Dict(loaded))
}

fn shutdown_timeout(config: &Entries, file: ConfigFile) -> Loaded<(Built, Built)> {
    let Some(value) = get(config, SHUTDOWN_TIMEOUT_KEY) else {
        return Ok((
            Built::Value(ConfigValue::Null),
            Built::Value(ConfigValue::Null),
        ));
    };
    let invalid = || {
        ConfigFailure::at(
            file,
            format!(
                "event_sinks.lifecycle.{SHUTDOWN_TIMEOUT_KEY} must be a fixed duration from '0s' to \
                 '10m' such as '30s'"
            ),
        )
    };
    let ConfigValue::String(text) = value else {
        return Err(invalid());
    };
    let seconds = fixed_duration_seconds(text, true)?.ok_or_else(invalid)?;
    let limit = SHUTDOWN_TIMEOUT_MAX_SECONDS.to_string();
    if compare_naturals(&seconds, &limit).is_gt() {
        return Err(invalid());
    }
    Ok((
        Built::Value(value.clone()),
        Built::Value(seconds_value(&seconds)),
    ))
}

fn lifecycle(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let config = match payload {
        None => return Ok(object("LifecycleEventSinksConfig", Vec::new())),
        Some(ConfigValue::Map(entries)) => entries,
        Some(_) => return Err(ConfigFailure::at(file, "event_sinks.lifecycle must be a mapping")),
    };
    let place = Place {
        file,
        label: "event_sinks.lifecycle",
    };
    validate_allowed_keys(config, &EVENT_SINK_KEYS, place)?;
    let named = named_filters(config, file)?;
    let defaults: Vec<(ConfigValue, ConfigValue)> = config
        .iter()
        .filter(|(key, _)| EVENT_FILTER_KEYS.contains(&key_name(key)))
        .cloned()
        .collect();
    let (timeout, timeout_seconds) = shutdown_timeout(config, file)?;
    Ok(object(
        "LifecycleEventSinksConfig",
        vec![
            ("defaults", load_filter(&defaults, place)?),
            ("named", named),
            ("shutdown_timeout", timeout),
            ("shutdown_timeout_seconds", timeout_seconds),
        ],
    ))
}

/// `_load_sinks`.
pub(crate) fn load_sinks(payload: Option<&ConfigValue>, file: ConfigFile) -> Loaded<Built> {
    let mapping = match payload {
        None => return Ok(object("SinksConfig", Vec::new())),
        Some(ConfigValue::Map(entries)) => entries,
        Some(_) => return Err(ConfigFailure::at(file, "event_sinks must be a mapping")),
    };
    validate_allowed_keys(
        mapping,
        &["lifecycle"],
        Place {
            file,
            label: "event_sinks",
        },
    )?;
    Ok(object(
        "SinksConfig",
        vec![("lifecycle", lifecycle(get(mapping, "lifecycle"), file)?)],
    ))
}
