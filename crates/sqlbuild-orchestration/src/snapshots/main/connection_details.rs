//! A target's publishable connection settings: named fields only, so no secret value can leak.

use serde_json::{Map, Value};

use crate::snapshots::constants::{
    AUTHENTICATOR_METHODS, CREDENTIAL_KEY_METHODS, DATABASE_KEYS, FILE_DATABASE_KEYS, HOST_KEYS,
    USER_KEYS,
};
use crate::snapshots::models::ConnectionDetails;

fn text<'a>(settings: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn first(settings: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| text(settings, key))
        .map(str::to_owned)
}

/// The sign-in method by name: Snowflake's authenticator, then which credential is set, or `none` for a file database.
fn auth_method(settings: &Map<String, Value>, file_database: bool) -> Option<String> {
    if let Some(authenticator) = text(settings, "authenticator") {
        let authenticator = authenticator.to_lowercase();
        return Some(
            AUTHENTICATOR_METHODS
                .iter()
                .find(|(name, _)| *name == authenticator)
                .map_or(authenticator.clone(), |(_, method)| (*method).to_owned()),
        );
    }
    CREDENTIAL_KEY_METHODS
        .iter()
        .find(|(key, _)| settings.get(*key).is_some_and(|value| !value.is_null()))
        .map(|(_, method)| (*method).to_owned())
        .or_else(|| file_database.then(|| "none".to_owned()))
}

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
