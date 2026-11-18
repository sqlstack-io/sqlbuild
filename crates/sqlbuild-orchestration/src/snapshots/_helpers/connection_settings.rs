//! Named, non-secret connection settings read from a target's settings.

use serde_json::{Map, Value};

use crate::snapshots::constants::{AUTHENTICATOR_METHODS, CREDENTIAL_KEY_METHODS};

/// A setting's trimmed text, when it is set and not blank.
fn text<'a>(settings: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    settings
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// The first of `keys` that is set, as text.
pub(crate) fn first(settings: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| text(settings, key))
        .map(str::to_owned)
}

/// The sign-in method by name: Snowflake's authenticator, then which credential is set, or `none` for a file database.
pub(crate) fn auth_method(settings: &Map<String, Value>, file_database: bool) -> Option<String> {
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
