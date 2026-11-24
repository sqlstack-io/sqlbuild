//! Python's `parse_loaded_schema_yml`: the seed declarations of one loaded schema YAML file.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::fields::{Site, get, reject_unknown_keys, truthy};
use crate::yaml_entries::_helpers::seeds::{mapping_list, seed_entry};
use crate::yaml_entries::constants::SEED_FILE_KEYS;
use crate::yaml_entries::models::ParsedSeedEntry;

/// Parse a loaded schema document; `in_seeds_directory` is whether `seeds` is among the path's parts.
pub fn parse_schema_document<N: AuthoredNode>(
    loaded: &N,
    path: &str,
    in_seeds_directory: bool,
) -> Result<Vec<ParsedSeedEntry<N>>, ConfigError> {
    let entries: Vec<(N, N)> = match loaded.kind() {
        NodeKind::Null => Vec::new(),
        NodeKind::Map => loaded.entries(),
        _ => {
            return Err(ConfigError::compile(format!(
                "{path} must contain a top-level mapping"
            )));
        }
    };
    let file: Site<'_> = Site { path, label: "" };
    if let Some(models) = get(&entries, "models") {
        if truthy(models) {
            return Err(ConfigError::compile(format!(
                "{path} declares model metadata in 'models', but model metadata must live in the \
                 model file MODEL(...). Move description, columns, and audits into the model \
                 header."
            )));
        }
        mapping_list(models, path, "models")?;
    }
    let seeds: Option<&N> = get(&entries, "seeds");
    if seeds.is_some_and(truthy) && !in_seeds_directory {
        return Err(ConfigError::compile(format!(
            "{path} declares seeds, but seed declarations must live under sql/seeds/**/*.yml"
        )));
    }
    if in_seeds_directory {
        reject_unknown_keys(
            &entries,
            &SEED_FILE_KEYS,
            Site {
                label: "seed file",
                ..file
            },
        )?;
    }
    let Some(seeds) = seeds else {
        return Ok(Vec::new());
    };
    mapping_list(seeds, path, "seeds")?
        .iter()
        .map(|seed| seed_entry(&seed.entries(), path))
        .collect()
}
