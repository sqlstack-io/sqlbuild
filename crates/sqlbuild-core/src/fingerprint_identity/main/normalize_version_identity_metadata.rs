//! Normalize recorded metadata without changing the persisted fingerprint.

use serde_json::{Map, Value};

use crate::fingerprint_identity::constants::{
    AUDIT_GATE_METADATA_KEY, DECLARED_COLUMNS_METADATA_KEY, DEFAULT_REFRESH_MODE,
    LOCAL_FUNCTION_HASHES_METADATA_KEY, MIGRATION_FINGERPRINT_METADATA_KEY,
};

const CONFIG_METADATA_KEY: &str = "config";
const REFRESH_MODE_CONFIG_KEY: &str = "refresh_mode";

pub fn normalize_version_identity_metadata(
    object: &mut Map<String, Value>,
    without_functions: bool,
) {
    for key in [
        AUDIT_GATE_METADATA_KEY,
        MIGRATION_FINGERPRINT_METADATA_KEY,
        DECLARED_COLUMNS_METADATA_KEY,
    ] {
        object.shift_remove(key);
    }
    if without_functions {
        object.shift_remove(LOCAL_FUNCTION_HASHES_METADATA_KEY);
    }
    if let Some(Value::Object(config)) = object.get_mut(CONFIG_METADATA_KEY)
        && config.get(REFRESH_MODE_CONFIG_KEY) == Some(&Value::from(DEFAULT_REFRESH_MODE))
    {
        config.shift_remove(REFRESH_MODE_CONFIG_KEY);
    }
}
