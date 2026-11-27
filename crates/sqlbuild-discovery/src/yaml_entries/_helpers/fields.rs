//! Python's authored-value field readers, with the errors their YAML callers raise.

use std::num::FpCategory;

use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::key_checks::main::unknown_mapping_keys::unknown_mapping_keys;

/// Where one YAML parse reports its errors: the file path and the label the messages name.
#[derive(Clone, Copy)]
pub(crate) struct Site<'a> {
    pub(crate) path: &'a str,
    pub(crate) label: &'a str,
}

impl Site<'_> {
    /// The error `<path> <label> <text>`.
    pub(crate) fn error(&self, text: &str) -> ConfigError {
        ConfigError::compile(format!("{} {} {text}", self.path, self.label))
    }
}

/// The value of the string key `key`, the last one when a mapping repeats it.
pub(crate) fn get<'a, N: AuthoredNode>(entries: &'a [(N, N)], key: &str) -> Option<&'a N> {
    entries
        .iter()
        .rev()
        .find(|(name, _)| name.is_text(key))
        .map(|(_, value)| value)
}

/// `get` that reads Python's `None` as absent.
fn present<'a, N: AuthoredNode>(entries: &'a [(N, N)], key: &str) -> Option<&'a N> {
    get(entries, key).filter(|value| value.kind() != NodeKind::Null)
}

fn non_blank<N: AuthoredNode>(node: &N) -> bool {
    node.with_text(|text| !python_strip(text).is_empty())
        .unwrap_or(false)
}

/// `require_non_empty_string`.
pub(crate) fn required_text<N: AuthoredNode>(
    entries: &[(N, N)],
    key: &str,
    site: Site<'_>,
) -> Result<N, ConfigError> {
    match get(entries, key) {
        Some(value) if non_blank(value) => Ok(value.clone()),
        _ => Err(site.error(&format!("must define non-empty string '{key}'"))),
    }
}

/// `optional_non_empty_string`.
pub(crate) fn optional_text<N: AuthoredNode>(
    entries: &[(N, N)],
    key: &str,
    site: Site<'_>,
) -> Result<Option<N>, ConfigError> {
    match present(entries, key) {
        None => Ok(None),
        Some(value) if non_blank(value) => Ok(Some(value.clone())),
        Some(_) => Err(site.error(&format!("'{key}' must be a non-empty string"))),
    }
}

/// `optional_bool`.
pub(crate) fn optional_bool<N: AuthoredNode>(
    entries: &[(N, N)],
    key: &str,
    site: Site<'_>,
) -> Result<Option<N>, ConfigError> {
    match present(entries, key) {
        None => Ok(None),
        Some(value) if matches!(value.kind(), NodeKind::Bool(_)) => Ok(Some(value.clone())),
        Some(_) => Err(site.error(&format!("'{key}' must be a boolean"))),
    }
}

/// `optional_mapping`: the authored mapping, or `None` for Python's empty default.
pub(crate) fn optional_mapping<N: AuthoredNode>(
    entries: &[(N, N)],
    key: &str,
    site: Site<'_>,
) -> Result<Option<N>, ConfigError> {
    match present(entries, key) {
        None => Ok(None),
        Some(value) if value.kind() == NodeKind::Map => Ok(Some(value.clone())),
        Some(_) => Err(site.error(&format!("'{key}' must be a mapping"))),
    }
}

/// `optional_string_tuple`.
pub(crate) fn optional_texts<N: AuthoredNode>(
    entries: &[(N, N)],
    key: &str,
    site: Site<'_>,
) -> Result<Vec<N>, ConfigError> {
    let Some(value) = present(entries, key) else {
        return Ok(Vec::new());
    };
    if value.kind() != NodeKind::List {
        return Err(site.error(&format!("'{key}' must be a list")));
    }
    let items: Vec<N> = value.items();
    if items.iter().any(|item| item.kind() != NodeKind::Str) {
        return Err(site.error(&format!("'{key}' entries must be strings")));
    }
    Ok(items)
}

/// `reject_unknown_mapping_keys`: the unknown keys and the nearest supported key as help.
pub(crate) fn reject_unknown_keys<N: AuthoredNode>(
    entries: &[(N, N)],
    allowed: &[&str],
    site: Site<'_>,
) -> Result<(), ConfigError> {
    let keys: Vec<(String, bool)> = entries
        .iter()
        .map(|(key, _)| (key.python_str(), key.kind() == NodeKind::Str))
        .collect();
    let allowed: Vec<String> = allowed.iter().map(|key| (*key).to_owned()).collect();
    match unknown_mapping_keys(&keys, &allowed) {
        None => Ok(()),
        Some((message, help)) => Err(site.error(&message).with_help(help)),
    }
}

/// Python's truth value of an authored value.
pub(crate) fn truthy<N: AuthoredNode>(node: &N) -> bool {
    match node.kind() {
        NodeKind::Null => false,
        NodeKind::Bool(value) => value,
        NodeKind::Int { .. } => node.integer().is_none_or(|value| value != 0),
        NodeKind::Str => node.with_text(|text| !text.is_empty()).unwrap_or(true),
        NodeKind::List | NodeKind::Tuple => !node.items().is_empty(),
        NodeKind::Map => !node.entries().is_empty(),
        NodeKind::Other => node
            .number()
            .is_none_or(|number| number.classify() != FpCategory::Zero),
    }
}
