//! The fingerprint's view of config files and authored sources.

use sqlbuild_config::models::ConfigValue;
use sqlbuild_config::toml::main::load_toml::load_toml;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};
use sqlbuild_core::posix_path::main::posix_suffix::posix_suffix;

use std::collections::HashSet;

use sqlbuild_discovery::tree::main::rglob::rglob;
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::scope_cache::_helpers::json::{canonical_json, object, sha256_hex};
use crate::scope_cache::constants::{
    LOCAL_CONFIG_FILENAME, LOCAL_CONFIG_KEYS, MACRO_SUFFIX, PROJECT_CONFIG_KEYS,
    RELATIONSHIP_ROOTS, SOURCE_SUFFIXES, TARGET_VARS_KEY, TARGETS_KEY, TOML_SUFFIX,
};
use crate::scope_cache::models::FingerprintInputs;
use crate::scope_index::constants::GLOBAL_ROLE_PARENT;

/// A config value `json.dumps` cannot serialize.
pub(crate) struct Unserializable;

/// Python's `Path(name).suffix` under the running Python.
fn suffix(path: &str) -> &str {
    posix_suffix(path)
}

/// `_source_role`: declaration, macro, relationship or resource.
pub(crate) fn source_role(relative: &str, inputs: &FingerprintInputs) -> &'static str {
    let parts: Vec<&str> = relative.split('/').collect();
    let role: &str = match parts.as_slice() {
        [parent, role, ..] if *parent == GLOBAL_ROLE_PARENT => role,
        _ => parts[0],
    };
    let mut declared: bool = inputs.named_declaration_directories.contains(&role.to_owned());
    for part in &parts {
        declared |= inputs.declaration_directories.contains(&(*part).to_owned());
    }
    if declared {
        return "declaration";
    }
    if suffix(relative) == MACRO_SUFFIX {
        return "macro";
    }
    if let [first, second, ..] = parts.as_slice()
        && RELATIONSHIP_ROOTS.contains(&(*first, *second))
    {
        return "relationship";
    }
    "resource"
}

fn json_value(value: &ConfigValue) -> Result<JsonValue, Unserializable> {
    Ok(match value {
        ConfigValue::Null => JsonValue::Null,
        ConfigValue::Bool(flag) => JsonValue::Bool(*flag),
        ConfigValue::Integer(number) => {
            JsonValue::Integer(JsonInteger::parse(&number.to_string()).ok_or(Unserializable)?)
        }
        ConfigValue::BigInteger(digits) => {
            JsonValue::Integer(JsonInteger::parse(digits).ok_or(Unserializable)?)
        }
        ConfigValue::Float(number) => JsonValue::Float(*number),
        ConfigValue::String(text) => JsonValue::String(text.clone()),
        ConfigValue::Date(_) | ConfigValue::DateTime(_) | ConfigValue::Time(_) => {
            return Err(Unserializable);
        }
        ConfigValue::List(items) => {
            JsonValue::Array(items.iter().map(json_value).collect::<Result<_, _>>()?)
        }
        ConfigValue::Map(entries) => {
            let mut fields: Vec<(String, JsonValue)> = Vec::with_capacity(entries.len());
            for (key, item) in entries {
                let ConfigValue::String(key) = key else {
                    return Err(Unserializable);
                };
                fields.push((key.clone(), json_value(item)?));
            }
            JsonValue::Object(fields)
        }
    })
}

/// `_scope_target_configs`: each named target's vars.
fn target_vars(value: &ConfigValue) -> Result<JsonValue, Unserializable> {
    let Some(entries) = value.as_map() else {
        return Ok(JsonValue::Object(Vec::new()));
    };
    let mut targets: Vec<(String, JsonValue)> = Vec::new();
    for (name, config) in entries {
        let (ConfigValue::String(name), Some(_)) = (name, config.as_map()) else {
            continue;
        };
        let vars: JsonValue = match config.get(TARGET_VARS_KEY) {
            Some(vars) => json_value(vars)?,
            None => JsonValue::Object(Vec::new()),
        };
        targets.push((name.clone(), object(vec![(TARGET_VARS_KEY, vars)])));
    }
    Ok(JsonValue::Object(targets))
}

/// `_scope_config_bytes`: a TOML config's scope-relevant keys as canonical JSON, else `raw`.
pub(crate) fn config_bytes(name: &str, raw: &[u8]) -> Result<Vec<u8>, Unserializable> {
    if !name.ends_with(TOML_SUFFIX) {
        return Ok(raw.to_vec());
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        return Ok(raw.to_vec());
    };
    let Ok(document) = load_toml(text) else {
        return Ok(raw.to_vec());
    };
    let Some(entries) = document.as_map() else {
        return Ok(raw.to_vec());
    };
    let keys: &[&str] = if name == LOCAL_CONFIG_FILENAME {
        &LOCAL_CONFIG_KEYS
    } else {
        &PROJECT_CONFIG_KEYS
    };
    let mut relevant: Vec<(String, JsonValue)> = Vec::new();
    for (key, value) in entries {
        let ConfigValue::String(key) = key else {
            continue;
        };
        if !keys.contains(&key.as_str()) {
            continue;
        }
        let value: JsonValue = if key == TARGETS_KEY {
            target_vars(value)?
        } else {
            json_value(value)?
        };
        relevant.push((key.clone(), value));
    }
    canonical_json(&JsonValue::Object(relevant))
        .map(String::into_bytes)
        .map_err(|_| Unserializable)
}

/// `(path, role, content hash)` of one hashed source.
pub(crate) type SourceRecord = (String, &'static str, String);

pub(crate) fn source_record(path: &str, role: &'static str, content: &[u8]) -> SourceRecord {
    (path.replace('\\', "/"), role, sha256_hex(content))
}

/// Every hashable authored source below `roots`, unsorted; `None` when one cannot be read.
pub(crate) fn source_records(
    project_dir: &std::path::Path,
    roots: &[String],
    inputs: &FingerprintInputs,
) -> Option<Vec<SourceRecord>> {
    let tree: ProjectTree = ProjectTree::new(project_dir);
    let mut seen: HashSet<String> = HashSet::new();
    let mut records: Vec<SourceRecord> = Vec::new();
    for root in roots {
        let Ok(paths) = rglob(&tree, root, |entry| !entry.is_dir) else {
            return None;
        };
        for relative in paths {
            if tree.is_undecodable(&relative) {
                return None;
            }
            let absolute = tree.absolute(&relative);
            if !absolute.is_file()
                || !SOURCE_SUFFIXES.contains(&suffix(&relative).to_lowercase().as_str())
                || !seen.insert(relative.clone())
            {
                continue;
            }
            let Ok(content) = std::fs::read(&absolute) else {
                return None;
            };
            records.push(source_record(
                &relative,
                source_role(&relative, inputs),
                &content,
            ));
        }
    }
    Some(records)
}
