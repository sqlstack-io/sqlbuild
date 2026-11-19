//! Drop a written-out default refresh mode, which plans the same as none.

use serde_json::{Map, Value};

use crate::fingerprint_identity::constants::{
    CONFIG_METADATA_KEY, DEFAULT_REFRESH_MODE, REFRESH_MODE_CONFIG_KEY,
};

pub fn without_default_refresh_mode(mut object: Map<String, Value>) -> Map<String, Value> {
    if let Some(Value::Object(config)) = object.get_mut(CONFIG_METADATA_KEY)
        && config.get(REFRESH_MODE_CONFIG_KEY) == Some(&Value::from(DEFAULT_REFRESH_MODE))
    {
        config.shift_remove(REFRESH_MODE_CONFIG_KEY);
    }
    object
}
