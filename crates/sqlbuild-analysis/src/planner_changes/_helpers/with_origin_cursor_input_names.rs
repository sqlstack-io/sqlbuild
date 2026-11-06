//! Name renamed cursor inputs by their origin models.

use serde_json::{Map, Value};

use crate::planner_changes::constants::{CONFIG_METADATA_KEY, CURSOR_INPUTS_CONFIG_KEY};
use sqlbuild_core::fingerprint_identity::main::{
    canonical_text::canonical_text, decoded_object::decoded_object,
};

/// Canonical metadata JSON whose `config.cursor_inputs` keys use origin names; other text is returned unchanged.
pub(crate) fn with_origin_cursor_input_names(
    metadata_json: &str,
    renamed_refs: &[(String, String)],
) -> String {
    let Ok(Some(mut object)) = decoded_object(metadata_json) else {
        return metadata_json.to_owned();
    };
    let Some(Value::Object(config)) = object.get(CONFIG_METADATA_KEY) else {
        return metadata_json.to_owned();
    };
    let Some(Value::Object(cursor_inputs)) = config.get(CURSOR_INPUTS_CONFIG_KEY) else {
        return metadata_json.to_owned();
    };
    let mut origin_inputs: Map<String, Value> = Map::new();
    for (name, value) in cursor_inputs {
        let origin: &str = renamed_refs
            .iter()
            .find(|(renamed, _)| renamed == name)
            .map_or(name.as_str(), |(_, origin)| origin.as_str());
        origin_inputs.insert(origin.to_owned(), value.clone());
    }
    let mut config: Map<String, Value> = config.clone();
    config.insert(
        CURSOR_INPUTS_CONFIG_KEY.to_owned(),
        Value::Object(origin_inputs),
    );
    object.insert(CONFIG_METADATA_KEY.to_owned(), Value::Object(config));
    canonical_text(object)
}
