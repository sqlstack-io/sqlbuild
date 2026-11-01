//! Decode the same immutable declaration documents at publication and execution boundaries.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::declarations::models::Definitions;
use crate::store::errors::ControlError;

fn document<T: DeserializeOwned>(
    documents: &BTreeMap<String, Value>,
    name: &str,
) -> Result<BTreeMap<String, T>, ControlError> {
    documents.get(name).map_or_else(
        || Ok(BTreeMap::new()),
        |value| {
            serde_json::from_value(value.clone())
                .map_err(|error| ControlError::Invalid(format!("{name}: {error}")))
        },
    )
}

pub(crate) fn decode_definitions(
    documents: &BTreeMap<String, Value>,
) -> Result<Definitions, ControlError> {
    let mut definitions = Definitions {
        jobs: document(documents, "jobs")?,
        groups: document(documents, "groups")?,
        ..Definitions::default()
    };
    for (name, value) in document::<Value>(documents, "triggers")? {
        if value.get("when").is_some() {
            definitions.sensors.insert(
                name,
                serde_json::from_value(value)
                    .map_err(|error| ControlError::Invalid(format!("triggers: {error}")))?,
            );
        } else {
            definitions.triggers.insert(
                name,
                serde_json::from_value(value)
                    .map_err(|error| ControlError::Invalid(format!("triggers: {error}")))?,
            );
        }
    }
    Ok(definitions)
}
