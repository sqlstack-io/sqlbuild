use sqlbuild_model_config::types::AuthoredNode;

use crate::yaml_entries::main::parse_schema_document::parse_schema_document;
use crate::yaml_entries::main::parse_sources_document::parse_sources_document;
use crate::yaml_entries::models::{ParsedSeedEntry, ParsedSourceEntry};
use crate::yaml_entries::tests::test_types::Node;

pub(super) const SCHEMA_PATH: &str = "/project/seeds/countries.yml";
pub(super) const SOURCES_PATH: &str = "/project/sources/shop.yml";

pub(super) fn text(value: &str) -> Node {
    Node::Str(value.to_owned())
}

pub(super) fn map(entries: Vec<(&str, Node)>) -> Node {
    Node::Map(
        entries
            .into_iter()
            .map(|(key, value)| (text(key), value))
            .collect(),
    )
}

/// A schema document declaring one `countries` seed with `columns` and extra keys.
pub(super) fn seed(columns: Vec<Node>, extra: Vec<(&str, Node)>) -> Node {
    let mut entries: Vec<(&str, Node)> = vec![
        ("name", text("countries")),
        ("columns", Node::List(columns)),
    ];
    entries.extend(extra);
    map(vec![("seeds", Node::List(vec![map(entries)]))])
}

/// A typed column.
pub(super) fn column(name: &str, column_type: &str) -> Node {
    map(vec![("name", text(name)), ("type", text(column_type))])
}

/// A sources document with one entry.
pub(super) fn sources(entry: Vec<(&str, Node)>) -> Node {
    map(vec![("sources", Node::List(vec![map(entry)]))])
}

/// A `dlt_sources` document with one group, which SQLBuild rejects.
pub(super) fn dlt_group(group: Vec<(&str, Node)>) -> Node {
    map(vec![("dlt_sources", Node::List(vec![map(group)]))])
}

fn shown(node: Option<&Node>) -> String {
    node.map_or_else(|| "-".to_owned(), AuthoredNode::python_str)
}

fn seed_line(seed: &ParsedSeedEntry<Node>) -> String {
    let tags: Vec<String> = seed.tags.iter().map(AuthoredNode::python_str).collect();
    format!(
        "{} columns={} tags={} csv_settings={}",
        seed.name.python_str(),
        seed.columns.len(),
        tags.join(","),
        seed.csv_settings.is_some()
    )
}

fn source_line(source: &ParsedSourceEntry<Node>) -> String {
    format!(
        "{} loader={} managed={} schema={} table={}{}{}",
        source.name.python_str(),
        source.loader.as_deref().unwrap_or("-"),
        source.managed,
        shown(source.schema.as_ref()),
        shown(source.table.as_ref()),
        if source.read_final { " final" } else { "" },
        source
            .load
            .as_deref()
            .map(|load| format!(" load={load}"))
            .unwrap_or_default()
    )
}

/// One line per parsed seed, or the parse error's message.
pub(super) fn schema_outcome(
    document: &Node,
    in_seeds_directory: bool,
) -> Result<Vec<String>, String> {
    parse_schema_document(document, SCHEMA_PATH, in_seeds_directory)
        .map(|seeds| seeds.iter().map(seed_line).collect())
        .map_err(|error| error.message)
}

/// One line per parsed source, or the parse error's message.
pub(super) fn sources_outcome(document: &Node) -> Result<Vec<String>, String> {
    parse_sources_document(document, SOURCES_PATH)
        .map(|document| {
            document
                .load
                .iter()
                .map(|load| format!("load {}", load.name))
                .chain(document.sources.iter().map(source_line))
                .collect()
        })
        .map_err(|error| error.message)
}

/// A sources document with a `load:` block and source entries.
pub(super) fn loaded_sources(load: Vec<(&str, Node)>, entries: Vec<Vec<(&str, Node)>>) -> Node {
    map(vec![
        ("load", map(load)),
        (
            "sources",
            Node::List(entries.into_iter().map(map).collect()),
        ),
    ])
}
