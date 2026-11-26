//! Python's `parse_loaded_sources_yml`: the source declarations and the load of one loaded sources YAML file.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::fields::{Site, get, reject_unknown_keys};
use crate::yaml_entries::_helpers::loads::{filling_load, sources_load};
use crate::yaml_entries::_helpers::removed_integrations::reject_dlt_sources;
use crate::yaml_entries::_helpers::sources::source_entry;
use crate::yaml_entries::constants::{DESTINATION_SCHEMA_KEY, SOURCE_FILE_KEYS};
use crate::yaml_entries::models::{ParsedLoad, ParsedSourceEntry, ParsedSourcesDocument};

/// Parse a loaded sources document's `sources` and `load`, linking each source the load fills to it.
pub fn parse_sources_document<N: AuthoredNode>(
    loaded: &N,
    path: &str,
) -> Result<ParsedSourcesDocument<N>, ConfigError> {
    let payload: Vec<(N, N)> = match loaded.kind() {
        NodeKind::Null => Vec::new(),
        NodeKind::Map => loaded.entries(),
        _ => {
            return Err(ConfigError::compile(format!(
                "{path} must contain a top-level mapping"
            )));
        }
    };
    reject_dlt_sources(&payload, path)?;
    reject_unknown_keys(
        &payload,
        &SOURCE_FILE_KEYS,
        Site {
            path,
            label: "sources file",
        },
    )?;
    let load: Option<ParsedLoad<N>> = sources_load(&payload, path)?;
    let load_name: Option<&str> = load.as_ref().map(|load| load.name.as_str());
    let destination_schema: Option<N> = load
        .as_ref()
        .and_then(|load| get(&load.block.entries(), DESTINATION_SCHEMA_KEY).cloned());
    let mut sources: Vec<ParsedSourceEntry<N>> = Vec::new();
    if let Some(raw) = get(&payload, "sources") {
        if raw.kind() != NodeKind::List {
            return Err(ConfigError::compile(format!(
                "{path} sources must be a list"
            )));
        }
        for item in raw.items() {
            if item.kind() != NodeKind::Map {
                return Err(ConfigError::compile(format!(
                    "{path} sources must contain only mappings"
                )));
            }
            let entries: Vec<(N, N)> = item.entries();
            let name: String = get(&entries, "name")
                .and_then(AuthoredNode::text)
                .unwrap_or_default();
            let load: Option<String> = filling_load(&entries, (&name, load_name), path)?;
            let mut source: ParsedSourceEntry<N> = source_entry(&entries, path)?;
            if load.is_some() {
                source.schema = destination_schema.clone();
            }
            source.load = load;
            sources.push(source);
        }
    }
    Ok(ParsedSourcesDocument { sources, load })
}
