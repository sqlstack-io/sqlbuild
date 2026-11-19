//! `DagNode` values of sources, loaders, seeds, functions, models and Python nodes.

use sqlbuild_core::text::main::python_strip::python_strip;
use std::collections::HashMap;

use crate::assembly::project::types::ObjectKey;
use crate::dag_artifact::_helpers::values::{
    column, object, optional_text, text, texts, typed_column,
};
use crate::dag_artifact::constants::{
    ASSET_KIND, DEFAULT_MATERIALIZATION_PYTHON_ASSET, LOADER_DESTINATION_PREFIX, LOADER_KIND,
    LOADER_MAX_PARTS, LOADER_NODE_META_KEY, MODEL_KIND, SCALAR_RETURN_KIND, SEED_KIND, SOURCE_KIND,
    TABLE_FUNCTION_KIND, TABLE_RETURN_KIND, UDF_KIND,
};
use crate::dag_artifact::errors::DagArtifactError;
use crate::dag_artifact::models::{
    DagColumnInput, DagFunctionInput, DagLoaderInput, DagModelInput, DagProjectInput,
    DagPythonNodeInput, DagRelationInput, DagSeedInput, DagSourceEntryInput, DagSourceInput,
};
use crate::dag_artifact::types::DagColumnLineage;
use crate::dag_artifact::types::DagValue;
use crate::python_values::main::text_key::text_key;

/// Every `DagNode` field, `None` or empty where a node kind leaves it out.
#[derive(Default)]
pub(crate) struct NodeFields {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) asset_key: Vec<String>,
    pub(crate) target: Option<DagValue>,
    pub(crate) path: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) sql: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) group: Option<String>,
    pub(crate) meta: Option<DagValue>,
    pub(crate) columns: Vec<DagValue>,
    pub(crate) column_lineage: DagColumnLineage,
    pub(crate) materialization_type: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) return_kind: Option<String>,
    pub(crate) returns: Option<String>,
    pub(crate) loader: Option<String>,
    pub(crate) arguments: Vec<(String, String)>,
}

impl NodeFields {
    /// The node as `dataclasses.asdict` writes it.
    pub(crate) fn value(self) -> DagValue {
        object(vec![
            ("id", text(&self.id)),
            ("kind", text(&self.kind)),
            ("name", text(&self.name)),
            (
                "asset_key",
                texts(self.asset_key.iter().map(String::as_str)),
            ),
            ("target", self.target.unwrap_or(DagValue::Null)),
            ("path", optional_text(self.path.as_deref())),
            ("description", optional_text(self.description.as_deref())),
            ("sql", optional_text(self.sql.as_deref())),
            ("tags", texts(self.tags.iter().map(String::as_str))),
            ("group", optional_text(self.group.as_deref())),
            ("meta", self.meta.unwrap_or(DagValue::Object(Vec::new()))),
            ("columns", DagValue::Array(self.columns)),
            (
                "column_lineage",
                DagValue::Object(
                    self.column_lineage
                        .iter()
                        .map(|(name, refs)| (text_key(name), lineage_refs(refs)))
                        .collect(),
                ),
            ),
            (
                "materialization_type",
                optional_text(self.materialization_type.as_deref()),
            ),
            ("language", optional_text(self.language.as_deref())),
            ("return_kind", optional_text(self.return_kind.as_deref())),
            ("returns", optional_text(self.returns.as_deref())),
            ("loader", optional_text(self.loader.as_deref())),
            (
                "arguments",
                DagValue::Array(
                    self.arguments
                        .iter()
                        .map(|(name, data_type)| {
                            object(vec![("name", text(name)), ("type", text(data_type))])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

fn lineage_refs(refs: &[(String, String)]) -> DagValue {
    DagValue::Array(
        refs.iter()
            .map(|(node, column)| object(vec![("node", text(node)), ("column", text(column))]))
            .collect(),
    )
}

/// `{resource type}:{name}`.
pub(crate) fn node_id((kind, name): &ObjectKey) -> String {
    format!("{kind}:{name}")
}

/// `loader:{name}`.
pub(crate) fn loader_node_id(name: &str) -> String {
    format!("{LOADER_KIND}:{name}")
}

/// A Python node's id: loaders share the loader namespace.
pub(crate) fn python_node_id(kind: &str, name: &str) -> String {
    if kind == LOADER_KIND {
        return loader_node_id(name);
    }
    format!("{kind}:{name}")
}

fn present(parts: &[Option<&str>]) -> Vec<String> {
    parts
        .iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .map(|part| (*part).to_owned())
        .collect()
}

fn entry_target(entry: (Option<&str>, Option<&str>, &str)) -> DagValue {
    let (database, schema, name) = entry;
    object(vec![
        ("database", optional_text(database)),
        ("schema", optional_text(schema)),
        ("name", text(name)),
        (
            "qualified_name",
            text(&present(&[database, schema, Some(name)]).join(".")),
        ),
        ("logical_database", DagValue::Null),
        ("logical_schema", DagValue::Null),
    ])
}

fn relation_target(target: &DagRelationInput) -> DagValue {
    object(vec![
        ("database", optional_text(target.database.as_deref())),
        ("schema", optional_text(target.schema.as_deref())),
        ("name", optional_text(target.name.as_deref())),
        (
            "qualified_name",
            optional_text(target.qualified_name.as_deref()),
        ),
        (
            "logical_database",
            optional_text(target.logical_database.as_deref()),
        ),
        (
            "logical_schema",
            optional_text(target.logical_schema.as_deref()),
        ),
    ])
}

fn relation_asset_key(target: &DagRelationInput) -> Vec<String> {
    let database: Option<&str> = target
        .logical_database
        .as_deref()
        .filter(|part| !part.is_empty())
        .or(target.database.as_deref());
    let schema: Option<&str> = target
        .logical_schema
        .as_deref()
        .filter(|part| !part.is_empty())
        .or(target.schema.as_deref());
    present(&[database, schema, target.name.as_deref()])
}

fn entry_table(entry: &DagSourceEntryInput) -> &str {
    entry
        .table
        .as_deref()
        .filter(|table| !table.is_empty())
        .unwrap_or(&entry.name)
}

fn columns(columns: &[DagColumnInput]) -> Vec<DagValue> {
    columns.iter().map(column).collect()
}

/// The source node.
pub(crate) fn source_node(source: &DagSourceInput) -> DagValue {
    let entry: &DagSourceEntryInput = &source.entry;
    let table: &str = entry_table(entry);
    let mut asset_key: Vec<String> = present(&[
        entry.database.as_deref(),
        entry.schema.as_deref(),
        Some(table),
    ]);
    if asset_key.is_empty() {
        asset_key.push(entry.name.clone());
    }
    NodeFields {
        id: node_id(&source.key),
        kind: SOURCE_KIND.to_owned(),
        name: source.name.clone(),
        asset_key,
        target: Some(entry_target((
            entry.database.as_deref(),
            entry.schema.as_deref(),
            table,
        ))),
        path: Some(source.path.clone()),
        description: entry.description.clone(),
        meta: Some(entry.meta.clone()),
        columns: columns(&entry.columns),
        ..NodeFields::default()
    }
    .value()
}

/// `loader_destination_parts`: `(database, schema, table)` of a one- to three-part destination.
fn destination_parts(
    destination: &str,
    defaults: (Option<&str>, Option<&str>),
) -> Result<(Option<String>, Option<String>, String), DagArtifactError> {
    let parts: Vec<&str> = destination.split('.').collect();
    if parts.len() > LOADER_MAX_PARTS || parts.iter().any(|part| python_strip(part).is_empty()) {
        return Err(DagArtifactError::LoaderDestination(format!(
            "Invalid loader destination '{destination}': expected 1 to 3 non-empty \
             dot-separated parts (table, schema.table, or database.schema.table)"
        )));
    }
    let owned = |part: Option<&str>| part.map(str::to_owned);
    Ok(match parts.as_slice() {
        [table] => (owned(defaults.0), owned(defaults.1), (*table).to_owned()),
        [schema, table] => (
            owned(defaults.0),
            Some((*schema).to_owned()),
            (*table).to_owned(),
        ),
        _ => (
            Some(parts[0].to_owned()),
            Some(parts[1].to_owned()),
            parts[2].to_owned(),
        ),
    })
}

/// The node of a loader no source declares.
pub(crate) fn loader_node(
    loader: &DagLoaderInput,
    project: &DagProjectInput,
) -> Result<DagValue, DagArtifactError> {
    let destination: String = loader
        .destination
        .clone()
        .unwrap_or_else(|| format!("{LOADER_DESTINATION_PREFIX}{}", loader.name));
    let (database, schema, table) = destination_parts(
        &destination,
        (
            project.target_database.as_deref(),
            project.target_schema.as_deref(),
        ),
    )?;
    let table: &str = if table.is_empty() {
        &loader.name
    } else {
        &table
    };
    Ok(NodeFields {
        id: loader_node_id(&loader.name),
        kind: LOADER_KIND.to_owned(),
        name: loader.name.clone(),
        asset_key: vec![loader.name.clone()],
        target: Some(entry_target((
            database.as_deref(),
            schema.as_deref(),
            table,
        ))),
        path: Some(loader.path.clone()),
        meta: Some(DagValue::Object(vec![(
            text_key(LOADER_NODE_META_KEY),
            DagValue::Bool(true),
        )])),
        columns: columns(&loader.columns),
        loader: Some(loader.name.clone()),
        ..NodeFields::default()
    }
    .value())
}

/// The seed node.
pub(crate) fn seed_node(seed: &DagSeedInput) -> DagValue {
    NodeFields {
        id: node_id(&seed.key),
        kind: SEED_KIND.to_owned(),
        name: seed.name.clone(),
        asset_key: relation_asset_key(&seed.destination),
        target: Some(relation_target(&seed.destination)),
        path: Some(seed.path.clone()),
        description: seed.description.clone(),
        tags: seed.tags.clone(),
        meta: Some(seed.meta.clone()),
        columns: columns(&seed.columns),
        ..NodeFields::default()
    }
    .value()
}

/// The UDF or table function node.
pub(crate) fn function_node(function: &DagFunctionInput) -> DagValue {
    let (kind, return_kind) = if function.return_columns.is_empty() {
        (UDF_KIND, SCALAR_RETURN_KIND)
    } else {
        (TABLE_FUNCTION_KIND, TABLE_RETURN_KIND)
    };
    NodeFields {
        id: format!("{kind}:{}", function.name),
        kind: kind.to_owned(),
        name: function.name.clone(),
        asset_key: relation_asset_key(&function.destination),
        target: Some(relation_target(&function.destination)),
        path: Some(function.path.clone()),
        sql: Some(function.body_sql.clone()),
        language: Some(function.language.clone()),
        tags: function.tags.clone(),
        return_kind: Some(return_kind.to_owned()),
        returns: Some(function.returns.clone()),
        columns: function.return_columns.iter().map(typed_column).collect(),
        arguments: function.arguments.clone(),
        ..NodeFields::default()
    }
    .value()
}

fn model_tags(model: &DagModelInput) -> Vec<String> {
    let schema_tags: Vec<String> = model
        .schema
        .as_ref()
        .map_or_else(Vec::new, |schema| schema.tags.clone());
    let Some(config_tags) = &model.config_tags else {
        return schema_tags;
    };
    let mut tags: Vec<String> = Vec::new();
    for tag in schema_tags.iter().chain(config_tags) {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    tags
}

fn model_columns(model: &DagModelInput) -> Vec<DagValue> {
    if let Some(schema) = model
        .schema
        .as_ref()
        .filter(|schema| !schema.columns.is_empty())
    {
        return columns(&schema.columns);
    }
    model
        .inferred_columns
        .as_ref()
        .map_or_else(Vec::new, |inferred| {
            inferred.iter().map(typed_column).collect()
        })
}

/// The model node.
pub(crate) fn model_node(model: &DagModelInput) -> DagValue {
    NodeFields {
        id: node_id(&model.key),
        kind: MODEL_KIND.to_owned(),
        name: model.name.clone(),
        asset_key: relation_asset_key(&model.destination),
        target: Some(relation_target(&model.destination)),
        path: Some(model.path.clone()),
        description: model
            .schema
            .as_ref()
            .and_then(|schema| schema.description.clone()),
        sql: Some(model.authored_sql.clone()),
        tags: model_tags(model),
        meta: model.schema.as_ref().map(|schema| schema.meta.clone()),
        columns: model_columns(model),
        materialization_type: Some(model.materialization.clone()),
        ..NodeFields::default()
    }
    .value()
}

/// The node of a Python asset or check.
pub(crate) fn python_node(node: &DagPythonNodeInput) -> DagValue {
    let mut fields: NodeFields = NodeFields {
        id: python_node_id(&node.kind, &node.name),
        kind: node.kind.clone(),
        name: node.name.clone(),
        asset_key: vec![node.kind.clone(), node.name.clone()],
        path: Some(node.path.clone()),
        description: node.description.clone(),
        tags: node.tags.clone(),
        group: node.group.clone(),
        meta: Some(node.meta.clone()),
        ..NodeFields::default()
    };
    if let Some(asset) = node.asset.as_ref().filter(|_| node.kind == ASSET_KIND) {
        fields.columns = columns(&asset.columns);
        fields.column_lineage = asset.column_lineage.clone().unwrap_or_default();
        fields.materialization_type = Some(DEFAULT_MATERIALIZATION_PYTHON_ASSET.to_owned());
    }
    fields.value()
}

/// Each loader's declaring source entry name, the last source naming a loader winning.
pub(crate) fn sources_by_loader(project: &DagProjectInput) -> HashMap<&str, &str> {
    let mut sources: HashMap<&str, &str> = HashMap::new();
    for source in &project.sources {
        if let Some(loader) = source.entry.loader.as_deref() {
            sources.insert(loader, source.entry.name.as_str());
        }
    }
    sources
}

/// The source node id of a loader a source declares, or its loader node id.
pub(crate) fn loader_or_source_id(name: &str, sources: &HashMap<&str, &str>) -> String {
    sources.get(name).map_or_else(
        || loader_node_id(name),
        |source| node_id(&(SOURCE_KIND.to_owned(), (*source).to_owned())),
    )
}
