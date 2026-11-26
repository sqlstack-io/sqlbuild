//! One `sources:` entry: its shape, managed loader, settings and the checks across them.

use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::header_metadata::main::parse_audit_list::parse_audit_list;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::yaml_entries::_helpers::columns::yaml_columns;
use crate::yaml_entries::_helpers::fields::{
    Site, get, optional_bool, optional_mapping, optional_text, reject_unknown_keys, required_text,
    stripped_strings,
};
use crate::yaml_entries::_helpers::freshness::freshness;
use crate::yaml_entries::_helpers::removed_integrations::reject_ingestr_source;
use crate::yaml_entries::constants::LOADER_OWNED_KEYS;
use crate::yaml_entries::models::{ParsedSourceEntry, ParsedYamlColumn};

const ENTRY_KEYS: [&str; 18] = [
    "audits",
    "columns",
    "contract",
    "cursor_column",
    "database",
    "description",
    "expression",
    "final",
    "freshness",
    "load_batch_size",
    "managed",
    "meta",
    "name",
    "schema",
    "table",
    "type_enforcement",
    "unique_key",
    "write_strategy",
];
const COLUMN_KEYS: [&str; 6] = ["audits", "description", "meta", "name", "nullable", "type"];
const CONTRACT_POLICIES: [&str; 2] = ["none", "enforced"];
const WRITE_STRATEGIES: [&str; 4] = ["append", "delete_insert", "merge", "table"];
const APPEND: &str = "append";
const DELETE_INSERT: &str = "delete_insert";
const MERGE: &str = "merge";
const TABLE: &str = "table";

/// What Python reads of an entry before its name: columns, typing, contract and expression.
struct SourceShape<N> {
    columns: Vec<ParsedYamlColumn<N>>,
    type_enforcement: Option<bool>,
    contract: Option<N>,
    expression: Option<N>,
}

/// `_parse_source_entry`: one entry of `sources`, read in Python's order.
pub(crate) fn source_entry<N: AuthoredNode>(
    entry: &[(N, N)],
    path: &str,
) -> Result<ParsedSourceEntry<N>, ConfigError> {
    let site: Site<'_> = Site {
        path,
        label: "source",
    };
    reject_ingestr_source(entry, site)?;
    let shape: SourceShape<N> = source_shape(entry, site)?;
    let name: N = required_text(entry, "name", site)?;
    let name_text: String = name.text().unwrap_or_default();
    let managed: ParsedSourceEntry<N> = managed_source(entry, name, &name_text, site)?;
    let parsed: ParsedSourceEntry<N> = with_settings(entry, site, shape, managed)?;
    validate_source(&parsed, &name_text, path)?;
    Ok(parsed)
}

fn source_shape<N: AuthoredNode>(
    entry: &[(N, N)],
    site: Site<'_>,
) -> Result<SourceShape<N>, ConfigError> {
    let columns: Vec<ParsedYamlColumn<N>> = yaml_columns(entry, site, Some(&COLUMN_KEYS))?;
    let authored_enforcement: Option<bool> = optional_bool(entry, "type_enforcement", site)?
        .map(|node| node.kind() == NodeKind::Bool(true));
    let contract: Option<N> = optional_text(entry, "contract", site)?;
    if contract
        .as_ref()
        .and_then(AuthoredNode::text)
        .is_some_and(|text| !CONTRACT_POLICIES.contains(&text.as_str()))
    {
        return Err(site.error("'contract' must be one of: enforced, none"));
    }
    let expression: Option<N> = optional_text(entry, "expression", site)?;
    let typed: bool = columns.iter().any(|column| column.column_type.is_some());
    let type_enforcement: Option<bool> = authored_enforcement.or_else(|| typed.then_some(true));
    if get(entry, "loader").is_some() {
        return Err(site.error(
            "'loader' is not supported; use managed: true and name the terminal loader after \
             the source",
        ));
    }
    reject_unknown_keys(entry, &ENTRY_KEYS, site)?;
    Ok(SourceShape {
        columns,
        type_enforcement,
        contract,
        expression,
    })
}

/// The entry named `name`, managed by the loader named after it.
fn managed_source<N: AuthoredNode>(
    entry: &[(N, N)],
    name: N,
    name_text: &str,
    site: Site<'_>,
) -> Result<ParsedSourceEntry<N>, ConfigError> {
    let managed: bool = optional_bool(entry, "managed", site)?
        .is_some_and(|node| node.kind() == NodeKind::Bool(true));
    let mut parsed: ParsedSourceEntry<N> = ParsedSourceEntry::named(name);
    parsed.managed = managed;
    parsed.loader = managed.then(|| name_text.to_owned());
    Ok(parsed)
}

/// `parsed` with its relation, load settings, metadata and audits read in Python's order.
fn with_settings<N: AuthoredNode>(
    entry: &[(N, N)],
    site: Site<'_>,
    shape: SourceShape<N>,
    mut parsed: ParsedSourceEntry<N>,
) -> Result<ParsedSourceEntry<N>, ConfigError> {
    let path: &str = site.path;
    if let Some(loader) = &parsed.loader {
        for (key, instead) in LOADER_OWNED_KEYS {
            if get(entry, key).is_some() {
                return Err(site.error(&format!(
                    "is filled by @loader '{loader}', so it can't set {key}; {instead}"
                )));
            }
        }
    }
    parsed.database = optional_text(entry, "database", site)?;
    parsed.schema = optional_text(entry, "schema", site)?;
    parsed.table = optional_text(entry, "table", site)?;
    parsed.read_final = optional_bool(entry, "final", site)?
        .is_some_and(|node| node.kind() == NodeKind::Bool(true));
    parsed.freshness = freshness(entry, &shape.columns, path)?;
    parsed.write_strategy = write_strategy(entry, site)?;
    parsed.load_batch_size = positive_int(entry, "load_batch_size", path)?;
    parsed.cursor_column = optional_text(entry, "cursor_column", site)?;
    parsed.unique_key = unique_key(entry, path)?;
    parsed.expression = shape.expression;
    parsed.description = optional_text(entry, "description", site)?;
    parsed.type_enforcement = shape.type_enforcement;
    parsed.contract = shape.contract;
    parsed.meta = optional_mapping(entry, "meta", site)?;
    parsed.columns = shape.columns;
    parsed.audits = parse_audit_list(get(entry, "audits"), path, "source", false)?;
    Ok(parsed)
}

fn write_strategy<N: AuthoredNode>(
    entry: &[(N, N)],
    site: Site<'_>,
) -> Result<Option<&'static str>, ConfigError> {
    let Some(text) = optional_text(entry, "write_strategy", site)?.and_then(|node| node.text())
    else {
        return Ok(None);
    };
    WRITE_STRATEGIES
        .iter()
        .copied()
        .find(|strategy| *strategy == text)
        .map(Some)
        .ok_or_else(|| {
            site.error(&format!(
                "write_strategy must be one of: {}",
                WRITE_STRATEGIES.join(", ")
            ))
        })
}

fn positive_int<N: AuthoredNode>(
    entry: &[(N, N)],
    key: &str,
    path: &str,
) -> Result<Option<N>, ConfigError> {
    let Some(value) = get(entry, key).filter(|value| value.kind() != NodeKind::Null) else {
        return Ok(None);
    };
    match value.kind() {
        NodeKind::Int { negative: false } if value.integer().is_none_or(|amount| amount != 0) => {
            Ok(Some(value.clone()))
        }
        _ => Err(ConfigError::compile(format!(
            "{path} source '{key}' must be a positive integer"
        ))),
    }
}

fn unique_key<N: AuthoredNode>(entry: &[(N, N)], path: &str) -> Result<Vec<String>, ConfigError> {
    let error = |text: &str| ConfigError::compile(format!("{path} source 'unique_key' {text}"));
    let Some(value) = get(entry, "unique_key").filter(|value| value.kind() != NodeKind::Null)
    else {
        return Ok(Vec::new());
    };
    let keys: Vec<String> = stripped_strings(value, error)?;
    if keys.is_empty() {
        return Err(error("must be non-empty"));
    }
    Ok(keys)
}

/// `_validate_source_entry`.
fn validate_source<N: AuthoredNode>(
    entry: &ParsedSourceEntry<N>,
    name: &str,
    path: &str,
) -> Result<(), ConfigError> {
    let error = |text: &str| ConfigError::compile(format!("{path} source '{name}' {text}"));
    let relation_keys: Vec<&str> = [
        ("database", &entry.database),
        ("schema", &entry.schema),
        ("table", &entry.table),
    ]
    .into_iter()
    .filter(|(_, value)| value.is_some())
    .map(|(key, _)| key)
    .collect();
    if entry.expression.is_some() && !relation_keys.is_empty() {
        return Err(error(&format!(
            "cannot define expression with {}",
            relation_keys.join(", ")
        )));
    }
    if entry.expression.is_some() && entry.read_final {
        return Err(error("cannot define final with expression"));
    }
    if entry.expression.is_some()
        && entry.type_enforcement == Some(true)
        && !entry
            .columns
            .iter()
            .any(|column| column.column_type.is_some())
    {
        return Err(error(
            "uses expression with type_enforcement but has no typed columns",
        ));
    }
    let strategy: Option<&str> = entry.write_strategy;
    let has_unique_key: bool = !entry.unique_key.is_empty();
    let has_cursor: bool = entry.cursor_column.is_some();
    if strategy.is_some() && entry.loader.is_none() {
        return Err(error("defines write_strategy but is not managed"));
    }
    if strategy == Some(APPEND) && has_unique_key {
        return Err(error(
            "unique_key is not supported with write_strategy append",
        ));
    }
    if strategy == Some(MERGE) && !has_unique_key {
        return Err(error("write_strategy merge requires unique_key"));
    }
    if strategy == Some(TABLE) && has_cursor {
        return Err(error(
            "cursor_column is not supported with write_strategy table",
        ));
    }
    if strategy == Some(TABLE) && has_unique_key {
        return Err(error(
            "unique_key is not supported with write_strategy table",
        ));
    }
    if strategy == Some(DELETE_INSERT) && !has_cursor {
        return Err(error("write_strategy delete_insert requires cursor_column"));
    }
    if strategy == Some(DELETE_INSERT) && has_unique_key {
        return Err(error(
            "unique_key is not supported with write_strategy delete_insert",
        ));
    }
    if has_cursor && !matches!(strategy, Some(APPEND | DELETE_INSERT | MERGE)) {
        return Err(error(
            "cursor_column requires write_strategy append, delete_insert, or merge",
        ));
    }
    if has_unique_key && strategy != Some(MERGE) {
        return Err(error("unique_key requires write_strategy merge"));
    }
    Ok(())
}
