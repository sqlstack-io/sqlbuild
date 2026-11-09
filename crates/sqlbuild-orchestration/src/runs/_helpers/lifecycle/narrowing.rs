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

/// The backfill window of a stored frozen request, or of an already projected record.
pub(crate) fn frozen_window<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RunWindow>, D::Error> {
    let value: Value = frozen(deserializer)?;
    let window: &Value = match value.get("window") {
        Some(window) => window,
        // A stored request always has its run ID; a projected window never does.
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

/// A JSON column, which SQLite returns as text and PostgreSQL as JSON.
fn frozen<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    let value: Value = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    Ok(match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
        other => other,
    })
}
