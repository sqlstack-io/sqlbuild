//! Shared manifest node pieces: fqn, columns, config, hooks and dependency ids.

use crate::assembly::project::types::ObjectKey;
use crate::manifest::_helpers::values::{
    docs, empty_array, empty_object, keyed, object, optional_text, text, text_or_empty, texts,
};
use crate::manifest::constants::{DEFAULT_RESOURCE_PREFIX, RESOURCE_TYPE_PREFIXES};
use crate::manifest::models::{
    ManifestColumnInput, ManifestConfigInput, ManifestHookInput, ManifestPathInput,
    ManifestPythonHookInput,
};
use crate::python_values::models::PythonValue;

/// `[project, *relative_path.parent.parts, relative_path.stem]`, with the parts read below `sql/`.
pub(crate) fn fqn(project_name: &str, path: &ManifestPathInput) -> PythonValue {
    let parents: &[String] = match path.parent_parts.split_first() {
        Some((first, rest)) if first == sqlbuild_core::constants::SQL_DIRECTORY => rest,
        _ => &path.parent_parts,
    };
    texts(
        std::iter::once(project_name)
            .chain(parents.iter().map(String::as_str))
            .chain(std::iter::once(path.stem.as_str())),
    )
}

/// The dbt-style columns dict, a later column replacing an earlier one of the same name.
pub(crate) fn columns(columns: &[ManifestColumnInput]) -> PythonValue {
    keyed(
        columns
            .iter()
            .map(|column| {
                (
                    column.name.clone(),
                    object(vec![
                        ("name", text(&column.name)),
                        ("description", text_or_empty(column.description.as_deref())),
                        ("meta", column.meta.clone().unwrap_or_else(empty_object)),
                        ("data_type", optional_text(column.data_type.as_deref())),
                        ("constraints", empty_array()),
                        ("quote", PythonValue::Null),
                        ("tags", empty_array()),
                    ]),
                )
            })
            .collect(),
    )
}

/// `prefix.project.name` of a compiled key.
pub(crate) fn unique_id((resource_type, name): &ObjectKey, project_name: &str) -> String {
    let prefix: &str = RESOURCE_TYPE_PREFIXES
        .iter()
        .find(|(kind, _)| kind == resource_type)
        .map_or(DEFAULT_RESOURCE_PREFIX, |(_, prefix)| prefix);
    format!("{prefix}.{project_name}.{name}")
}

/// The unique ids of `keys`.
pub(crate) fn unique_ids(keys: &[ObjectKey], project_name: &str) -> PythonValue {
    PythonValue::Array(
        keys.iter()
            .map(|key| text(&unique_id(key, project_name)))
            .collect(),
    )
}

fn dbt_sql_hooks(hooks: Option<&[ManifestHookInput]>) -> PythonValue {
    let statements = hooks
        .unwrap_or_default()
        .iter()
        .filter_map(|hook| match hook {
            ManifestHookInput::Sql { statement, .. } => Some(statement),
            _ => None,
        });
    PythonValue::Array(
        statements
            .enumerate()
            .map(|(index, statement)| {
                object(vec![
                    ("sql", text(statement)),
                    ("transaction", PythonValue::Bool(true)),
                    (
                        "index",
                        PythonValue::Integer(sqlbuild_core::json::models::JsonInteger::from(
                            i64::try_from(index).unwrap_or(i64::MAX),
                        )),
                    ),
                ])
            })
            .collect(),
    )
}

/// The config tags, or an empty list.
pub(crate) fn config_tags(config: &ManifestConfigInput) -> PythonValue {
    texts(config.tags.iter().flatten().map(String::as_str))
}

/// The dbt-compatible config dict of a model.
pub(crate) fn config(config: &ManifestConfigInput) -> PythonValue {
    object(vec![
        ("enabled", config.enabled.clone()),
        ("alias", config.alias.clone()),
        ("schema", config.schema.clone()),
        ("database", config.database.clone()),
        ("tags", config_tags(config)),
        ("meta", empty_object()),
        ("group", PythonValue::Null),
        ("materialized", text(&config.materialized)),
        ("incremental_strategy", config.incremental_strategy.clone()),
        (
            "merge_exclude_columns",
            config.merge_exclude_columns.clone(),
        ),
        ("persist_docs", empty_object()),
        ("post-hook", dbt_sql_hooks(config.post_hooks.as_deref())),
        ("pre-hook", dbt_sql_hooks(config.pre_hooks.as_deref())),
        ("quoting", empty_object()),
        ("column_types", empty_object()),
        ("full_refresh", config.full_refresh.clone()),
        ("unique_key", config.unique_key.clone()),
        ("on_schema_change", config.on_schema_change.clone()),
        ("on_configuration_change", text("apply")),
        ("grants", empty_object()),
        ("packages", empty_array()),
        ("docs", docs()),
        (
            "contract",
            object(vec![
                ("enforced", PythonValue::Bool(false)),
                ("alias_types", PythonValue::Bool(true)),
            ]),
        ),
    ])
}

fn optional_field(name: &str, value: Option<PythonValue>) -> Option<(String, PythonValue)> {
    value.map(|value| (name.to_owned(), value))
}

/// `serialize_hook_entries` with every SQL hook output field and the Python hook identities.
pub(crate) fn lifecycle_hooks(
    hooks: Option<&[ManifestHookInput]>,
    python_hooks: &[ManifestPythonHookInput],
) -> Vec<PythonValue> {
    let mut serialized: Vec<PythonValue> = Vec::new();
    for hook in hooks.unwrap_or_default() {
        match hook {
            ManifestHookInput::Sql {
                statement,
                name,
                relative_path,
                definition_sql,
                kwargs,
                description,
            } => {
                let fields: Vec<(String, PythonValue)> = [
                    Some(("type".to_owned(), text("sql"))),
                    Some(("statement".to_owned(), text(statement))),
                    optional_field("name", name.as_deref().map(text)),
                    optional_field("relative_path", relative_path.as_deref().map(text)),
                    optional_field("definition_sql", definition_sql.as_deref().map(text)),
                    optional_field(
                        "kwargs",
                        (*kwargs != PythonValue::Null).then(|| kwargs.clone()),
                    ),
                    optional_field("description", description.as_deref().map(text)),
                ]
                .into_iter()
                .flatten()
                .collect();
                serialized.push(keyed(fields));
            }
            ManifestHookInput::Python { name, kwargs } => {
                let mut fields: Vec<(String, PythonValue)> = vec![
                    ("type".to_owned(), text("python")),
                    ("name".to_owned(), text(name)),
                    ("kwargs".to_owned(), kwargs.clone()),
                ];
                if let Some(identity) = python_hooks.iter().rev().find(|hook| hook.name == *name) {
                    fields.extend([
                        ("relative_path".to_owned(), text(&identity.relative_path)),
                        (
                            "definition_hash".to_owned(),
                            text(&identity.definition_hash),
                        ),
                        ("version_hash".to_owned(), text(&identity.version_hash)),
                    ]);
                }
                serialized.push(keyed(fields));
            }
            ManifestHookInput::Other => {}
        }
    }
    serialized
}

/// `{"macros": [], "nodes": [...]}`.
pub(crate) fn depends_on(deps: &[ObjectKey], project_name: &str) -> PythonValue {
    object(vec![
        ("macros", empty_array()),
        ("nodes", unique_ids(deps, project_name)),
    ])
}
