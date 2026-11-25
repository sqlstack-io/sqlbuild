//! A run's narrowing and backfill window are read from its frozen request, and nothing else of that request is returned.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::runs::models::{RunNarrowing, RunWindow};

/// The narrowing of a stored frozen request, which reruns copy, or of an already projected record.
pub(crate) fn frozen_narrowing<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RunNarrowing>, D::Error> {
    let value: Value = frozen(deserializer)?;
    let fields: &Value = value.get("narrowing").unwrap_or(&value);
    let tokens = |name: &str| -> Vec<String> {
        fields
            .get(name)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|token| token.as_str().map(str::to_owned))
            .collect()
    };
    let narrowing = RunNarrowing {
        select: tokens("select"),
        exclude: tokens("exclude"),
    };
    Ok((!narrowing.select.is_empty() || !narrowing.exclude.is_empty()).then_some(narrowing))
}

/// The backfill window of a stored frozen request, or of an already projected record: a stored request always has its run ID, and a projected window never does.
pub(crate) fn frozen_window<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RunWindow>, D::Error> {
    let value: Value = frozen(deserializer)?;
    let window: &Value = match value.get("window") {
        Some(window) => window,
        None if value.get("run_id").is_none() => &value,
        None => return Ok(None),
    };
    if window.is_null() {
        return Ok(None);
    }
    serde_json::from_value(window.clone())
        .map(Some)
        .map_err(serde::de::Error::custom)
}

/// The member attempt of a stored frozen request, or of an already projected record.
pub(crate) fn frozen_member_attempt<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u8>, D::Error> {
    let value: Value = frozen(deserializer)?;
    let attempt: &Value = value.get("member_attempt").unwrap_or(&value);
    attempt
        .as_u64()
        .map(|attempt| u8::try_from(attempt).map_err(serde::de::Error::custom))
        .transpose()
}

/// The command tokens of a stored authored request, or of an already projected record; `None` when the request has none.
pub(crate) fn authored_command<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    let value: Value = frozen(deserializer)?;
    let command: &Value = value
        .get("request")
        .and_then(|request| request.get("command"))
        .unwrap_or(&value);
    Ok(command.as_array().and_then(|tokens| {
        tokens
            .iter()
            .map(|token| token.as_str().map(str::to_owned))
            .collect()
    }))
}

/// A JSON column, which SQLite returns as text and PostgreSQL as JSON.
fn frozen<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    let value: Value = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    Ok(match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
        other => other,
    })
}
