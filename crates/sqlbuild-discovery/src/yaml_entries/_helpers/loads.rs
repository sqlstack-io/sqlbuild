//! A sources file's `load:` and the source entries it fills.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::fields::{Site, get, required_text};
use crate::yaml_entries::_helpers::sources::source_entry;
use crate::yaml_entries::constants::{LOAD_KEY, LOAD_NAME_SEPARATOR, LOAD_OWNED_KEYS};
use crate::yaml_entries::models::{ParsedLoad, ParsedSourceEntry};

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
fn filling_load<N: AuthoredNode>(
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

/// Refuse an `on_schema_change` on the source `name` when no load or `@loader` fills it.
fn refuse_unfilled_schema_policy<N: AuthoredNode>(
    source: &ParsedSourceEntry<N>,
    (name, load): (&str, Option<&str>),
    path: &str,
) -> Result<(), ConfigError> {
    if load.is_some() || source.managed || source.on_schema_change.is_none() {
        return Ok(());
    }
    Err(ConfigError::compile(format!(
        "{path} source '{name}' can't set on_schema_change; it says how the load or @loader \
         that fills a source meets new and missing columns, and nothing fills this one"
    )))
}

/// One `sources:` entry, linked to the file's load when it fills it, which then names its schema.
pub(crate) fn document_source<N: AuthoredNode>(
    entries: &[(N, N)],
    (load_name, destination_schema): (Option<&str>, Option<&N>),
    path: &str,
) -> Result<ParsedSourceEntry<N>, ConfigError> {
    let name: String = get(entries, "name")
        .and_then(AuthoredNode::text)
        .unwrap_or_default();
    let load: Option<String> = filling_load(entries, (&name, load_name), path)?;
    let mut source: ParsedSourceEntry<N> = source_entry(entries, path)?;
    if load.is_some() {
        source.schema = destination_schema.cloned();
    }
    refuse_unfilled_schema_policy(&source, (&name, load.as_deref()), path)?;
    source.load = load;
    Ok(source)
}
