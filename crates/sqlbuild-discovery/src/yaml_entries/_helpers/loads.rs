//! A sources file's `load:` and the source entries it fills.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::fields::{Site, get, required_text};
use crate::yaml_entries::constants::{LOAD_KEY, LOAD_NAME_SEPARATOR, LOAD_OWNED_KEYS};
use crate::yaml_entries::models::ParsedLoad;

/// The file's `load:`, its `name` checked; SQLLoad reads the rest.
pub(crate) fn sources_load<N: AuthoredNode>(
    payload: &[(N, N)],
    path: &str,
) -> Result<Option<ParsedLoad<N>>, ConfigError> {
    let Some(block) = get(payload, LOAD_KEY) else {
        return Ok(None);
    };
    if block.kind() != NodeKind::Map {
        return Err(ConfigError::compile(format!(
            "{path} load must be a mapping of SQLLoad source settings with a name"
        )));
    }
    let site = Site {
        path,
        label: "load",
    };
    let name: String = required_text(&block.entries(), "name", site)?
        .text()
        .unwrap_or_default();
    Ok(Some(ParsedLoad {
        name,
        block: block.clone(),
    }))
}

/// The load that fills the source `name` (its name is `{load}__{table}`), refusing the keys the load decides.
pub(crate) fn filling_load<N: AuthoredNode>(
    entry: &[(N, N)],
    (name, load): (&str, Option<&str>),
    path: &str,
) -> Result<Option<String>, ConfigError> {
    let Some(load) = load else {
        return Ok(None);
    };
    let filled = name
        .strip_prefix(load)
        .and_then(|rest| rest.strip_prefix(LOAD_NAME_SEPARATOR))
        .is_some_and(|table| !table.is_empty());
    if !filled {
        return Ok(None);
    }
    for (key, instead) in LOAD_OWNED_KEYS {
        if get(entry, key).is_some() {
            return Err(ConfigError::compile(format!(
                "{path} source '{name}' is filled by the load '{load}', so it can't set {key}; \
                 {instead}"
            )));
        }
    }
    Ok(Some(load.to_owned()))
}
