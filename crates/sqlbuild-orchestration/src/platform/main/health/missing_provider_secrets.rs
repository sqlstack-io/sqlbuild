//! The providers a published snapshot shows missing required secrets.

use serde_json::Value;

use crate::platform::models::{MissingProviderSecrets, MissingProviderSetting};

fn texts(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// Each provider of a snapshot's `providers` document with a `missing` setting; `allowed` names the target's opt-outs.
#[must_use]
pub fn missing_provider_secrets(
    providers: &Value,
    allowed: &[String],
) -> Vec<MissingProviderSecrets> {
    let mut missing: Vec<MissingProviderSecrets> = Vec::new();
    for provider in providers["providers"].as_array().into_iter().flatten() {
        let settings: Vec<MissingProviderSetting> = provider["settings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|setting| setting["missing"].as_bool().unwrap_or_default())
            .map(|setting| MissingProviderSetting {
                field: setting["name"].as_str().unwrap_or_default().to_owned(),
                variable: setting["env"].as_str().unwrap_or_default().to_owned(),
                line: setting["line"]
                    .as_u64()
                    .and_then(|line| u32::try_from(line).ok()),
            })
            .collect();
        if settings.is_empty() {
            continue;
        }
        let name: String = provider["name"].as_str().unwrap_or_default().to_owned();
        missing.push(MissingProviderSecrets {
            allowed: allowed.contains(&name),
            provider: name,
            path: provider["path"].as_str().unwrap_or_default().to_owned(),
            settings,
            nodes: texts(&provider["nodes"]),
        });
    }
    missing.sort_by(|left, right| left.provider.cmp(&right.provider));
    missing
}
