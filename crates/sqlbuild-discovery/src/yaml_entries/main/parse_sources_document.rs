//! Python's `parse_loaded_sources_yml`: the source declarations of one loaded sources YAML file.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::fields::{Site, get, reject_unknown_keys};
use crate::yaml_entries::_helpers::removed_integrations::reject_dlt_sources;
use crate::yaml_entries::_helpers::sources::source_entry;
use crate::yaml_entries::constants::SOURCE_FILE_KEYS;
use crate::yaml_entries::models::ParsedSourceEntry;

/// Parse a loaded sources document's `sources`.
pub fn parse_sources_document<N: AuthoredNode>(
    loaded: &N,
    path: &str,
) -> Result<Vec<ParsedSourceEntry<N>>, ConfigError> {
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
            sources.push(source_entry(&item.entries(), path)?);
        }
    }
    Ok(sources)
}
