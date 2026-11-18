//! A target's publishable connection settings: named fields only, so no secret value can leak.

use serde_json::{Map, Value};

use crate::snapshots::_helpers::connection_settings::{auth_method, first};
use crate::snapshots::constants::{DATABASE_KEYS, FILE_DATABASE_KEYS, HOST_KEYS, USER_KEYS};
use crate::snapshots::models::ConnectionDetails;

/// Only these named, non-secret settings are read; every other key, including every credential value, is ignored.
#[must_use]
pub fn connection_details(adapter: &str, settings: &Map<String, Value>) -> ConnectionDetails {
    let file_key = FILE_DATABASE_KEYS
        .iter()
        .find(|(name, _)| *name == adapter)
        .map(|(_, key)| *key);
    ConnectionDetails {
        adapter: adapter.to_owned(),
        account: first(settings, &["account"]),
        host: match file_key {
            Some(key) => first(settings, &[key]),
            None => first(settings, &HOST_KEYS),
        },
        user: first(settings, &USER_KEYS),
        role: first(settings, &["role"]),
        warehouse: first(settings, &["warehouse"]),
        database: file_key
            .is_none()
            .then(|| first(settings, &DATABASE_KEYS))
            .flatten(),
        auth_method: auth_method(settings, file_key.is_some()),
    }
}
