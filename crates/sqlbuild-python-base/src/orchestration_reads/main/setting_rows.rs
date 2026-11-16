//! Declared fields with the environment variable each is read from, which pydantic-settings derives from the prefix unless an alias names it.

use crate::orchestration_reads::_helpers::provider_settings;
use serde_json::Value;

/// Declared fields with the environment variable each is read from, which pydantic-settings derives from the prefix unless an alias names it.
#[inline]
pub fn setting_rows(class: &Value) -> Vec<Value> {
    provider_settings::setting_rows(class)
}
