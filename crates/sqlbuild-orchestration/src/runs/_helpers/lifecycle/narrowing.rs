//! A job run's narrowing is read from what its requester authored, and nothing else of that request is returned.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::runs::models::RunNarrowing;

/// The selectors a narrowed job run was requested with, from a stored authored request or an already projected narrowing.
pub(crate) fn authored_narrowing<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<RunNarrowing>, D::Error> {
    let value: Value = Option::<Value>::deserialize(deserializer)?.unwrap_or(Value::Null);
    let value: Value = match value {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
        other => other,
    };
    let fields: &Value = value.get("request").unwrap_or(&value);
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
