//! `resource_cost_observation_id`: the content-addressed ID of one cost observation.

use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_core::json::models::JsonValue;

use crate::events::_helpers::identity::{json_digest, optional_text, text};
use crate::resource_costs::constants::RESOURCE_COST_TAG;
use crate::resource_costs::models::ResourceCostKey;

/// SHA-256 of the compact JSON array of the key, so a retried append writes the same row.
pub fn resource_cost_observation_id(key: &ResourceCostKey<'_>) -> Result<String, JsonEmitError> {
    let value = JsonValue::Array(vec![
        text(RESOURCE_COST_TAG),
        text(key.run_id),
        text(&key.attempt.to_string()),
        text(key.resource_type),
        text(key.resource_name),
        text(key.warehouse_name),
        text(key.warehouse_size),
        optional_text(key.warehouse_type),
        optional_text(
            key.cluster_number
                .map(|number| number.to_string())
                .as_deref(),
        ),
    ]);
    json_digest(&value, false)
}
