//! The definition snapshot of a compiled project: one fingerprint per node and its declarations.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use serde_json::{Map, Value};
use sqlbuild_analysis::definition_changes::constants::{
    AUDITS_ASPECT, COLUMNS_ASPECT, CONFIG_ASPECT, CONNECTION_KIND, DEFINITION_ASPECT,
    DEFINITION_SNAPSHOT_FORMAT, DEPENDENCIES_KIND, FILE_ASPECT, MACRO_KIND, MATERIALIZATION_KIND,
    MODEL_KIND, PROJECT_NODE_NAME, PYTHON_NODE_KINDS, SCENARIO_KIND, SEED_KIND, SETTINGS_KIND,
    SNAPSHOTS_ASPECT, SOURCE_KIND, SQL_ASPECT, TEST_FILE_KIND, VARS_ASPECT,
};
use sqlbuild_analysis::definition_changes::main::definition_digest::definition_digest;
use sqlbuild_analysis::definition_changes::main::node_key::node_key;
use sqlbuild_analysis::definition_changes::main::sql_definition_digest::sql_definition_digest;
use sqlbuild_analysis::definition_changes::main::sql_macro_definition_digest::sql_macro_definition_digest;
use sqlbuild_analysis::definition_changes::models::{DefinitionSnapshot, NodeDefinition};

use crate::bindings::_helpers::change_selection::fingerprints::{
    Fingerprinting, directory_digest, file_digest, text_of,
};
use crate::bindings::_helpers::compiled_views::function_reads;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::seed_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::planner_identity::metadata::{function_local_hashes, seed_identity};
use crate::bindings::_helpers::records::fields::field as native_field;

/// Header values that rename rather than redefine a model.
const RENAME_CONFIG_KEYS: [&str; 2] = ["migrate_from", "description"];
const MIGRATE_FROM_KEY: &str = "migrate_from";
const MATERIALIZED_KEY: &str = "materialized";
/// Which header keys were authored, rather than inherited from defaults, does not change a model.
const AUTHORED_KEYS_FIELD: &str = "model_header_keys";
const SOURCE_EXPRESSION_FIELD: &str = "expression";
const SCENARIO_SNAPSHOT_ROOT: [&str; 2] = ["tests", "_scenario_snapshots"];
const CONTRACT_KEY: &str = "contract";
/// Source declaration fields that shape a source mock's columns.
const SOURCE_SHAPE_FIELDS: [&str; 3] = ["columns", "contract", "type_enforcement"];
/// Project-local adapter modules.
const ADAPTERS_DIRECTORY: &str = "adapters";
/// Project configuration fields that other nodes fingerprint, or that do not shape compiled SQL.
const PROJECT_CONFIG_ELSEWHERE: [&str; 11] = [
    "name",
    "settings",
    "vars",
    "defaults",
    "path_defaults",
    "cost",
    "diff",
    "janitor",
    "migrations",
    "sinks",
    "scenario",
];
const LOCAL_CONFIG_ELSEWHERE: [&str; 4] = ["settings", "setting_overrides", "vars", "scenario"];
/// Settings that tune how a run executes or validates, not what a node computes.
const RUNTIME_SETTINGS: [&str; 10] = [
    "sql_analysis",
    "require_sql_analysis",
    "strict",
    "concurrency",
    "microbatch_concurrency",
    "test_timeout",
    "test_timeout_seconds",
    "scenario_timeout",
    "scenario_timeout_seconds",
    "delta_compile",
];
const PYTHON_SUFFIX: &str = ".py";
/// Discovered Python node collections, in `PYTHON_NODE_KINDS` order.
const PYTHON_NODE_COLLECTIONS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];

fn node(kind: &str, name: &str, aspects: Vec<(&str, String)>) -> NodeDefinition {
    NodeDefinition {
        kind: kind.to_owned(),
        name: name.to_owned(),
        aspects: aspects
            .into_iter()
            .map(|(aspect, digest)| (aspect.to_owned(), digest))
            .collect(),
        uses: Vec::new(),
        migrate_from: Vec::new(),
    }
}

/// The node key of a scope declaration identity: `kind:name`, with `@owner` for owned ones.
pub(crate) fn declaration_key(identity: &Bound<'_, PyAny>) -> PyResult<String> {
    let kind: String = text_of(&native_field(&native_field(identity, "kind")?, "value")?)?;
    let name: String = native_field(identity, "name")?.extract()?;
    let owner: Bound<'_, PyAny> = native_field(identity, "owner")?;
    if owner.is_none() {
        return Ok(node_key(&kind, &name));
    }
    let owner_kind: String = text_of(&native_field(&native_field(&owner, "kind")?, "value")?)?;
    let owner_name: String = native_field(&owner, "name")?.extract()?;
    Ok(node_key(
        &kind,
        &format!("{name}@{}", node_key(&owner_kind, &owner_name)),
    ))
}

/// `kind:name` of a usage consumer.
fn consumer_key(consumer: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(node_key(
        &text_of(&native_field(&native_field(consumer, "kind")?, "value")?)?,
        &native_field(consumer, "name")?.extract::<String>()?,
    ))
}

/// Declaration keys each consumer uses, by consumer key, from the project's scope index.
pub(crate) fn declaration_uses(
    project: &Bound<'_, PyAny>,
) -> PyResult<BTreeMap<String, BTreeSet<String>>> {
    let mut uses: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let index: Bound<'_, PyAny> = native_field(project, "scope_index")?;
    if index.is_none() {
        return Ok(uses);
    }
    for usage in native_field(&index, "usages")?.try_iter()? {
        let usage: Bound<'_, PyAny> = usage?;
        uses.entry(consumer_key(&native_field(&usage, "consumer")?)?)
            .or_default()
            .insert(declaration_key(&native_field(&usage, "declaration")?)?);
    }
    Ok(uses)
}

/// A declaration file's digest: Python with imports, a SQL macro's definition, or normalised SQL.
fn declaration_file_digest(
    fingerprints: &Fingerprinting,
    path: &str,
    sql_macro: Option<&str>,
) -> String {
    if path.ends_with(PYTHON_SUFFIX) {
        return fingerprints.python_module_digest(Path::new(path));
    }
    match std::fs::read_to_string(fingerprints.project_dir.join(path)) {
        Ok(contents) => {
            let definition: String = match sql_macro {
                Some(name) => sql_macro_definition_digest(name, path, &contents),
                None => sql_definition_digest(&contents),
            };
            let mut parts: Vec<String> = vec![definition];
            parts.extend(fingerprints.referenced_vars_digest(&contents));
            let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
            definition_digest(&borrowed)
        }
        Err(_) => definition_digest(&["missing"]),
    }
}

/// Scope declarations by their file, a Python macro by its facts too, with the macros it calls.
fn declaration_nodes(
    project: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<BTreeMap<String, NodeDefinition>> {
    let mut nodes: BTreeMap<String, NodeDefinition> = BTreeMap::new();
    let index: Bound<'_, PyAny> = native_field(project, "scope_index")?;
    if index.is_none() {
        return Ok(nodes);
    }
    for declaration in native_field(&index, "declarations")?.try_iter()? {
        let declaration: Bound<'_, PyAny> = declaration?;
        let identity: Bound<'_, PyAny> = native_field(&declaration, "identity")?;
        let kind: String = text_of(&native_field(&native_field(&identity, "kind")?, "value")?)?;
        let name: String = native_field(&identity, "name")?.extract()?;
        let path: String = native_field(&declaration, "path")?.extract()?;
        let macro_facts: Bound<'_, PyAny> = native_field(&declaration, "macro")?;
        let sql_macro: Option<&str> =
            (kind == MACRO_KIND && !path.ends_with(PYTHON_SUFFIX)).then_some(name.as_str());
        let (digest, uses): (String, Vec<String>) = if kind == MACRO_KIND && !macro_facts.is_none()
        {
            let dependencies: Vec<String> = native_field(&macro_facts, "dependencies")?
                .try_iter()?
                .map(|dependency| declaration_key(&dependency?))
                .collect::<PyResult<_>>()?;
            let file: String = declaration_file_digest(fingerprints, &path, sql_macro);
            let digest: String = if sql_macro.is_some() {
                file
            } else {
                definition_digest(&[
                    &fingerprints.canonical_digest(&macro_facts, (&[], &[]))?,
                    &file,
                ])
            };
            (digest, dependencies)
        } else {
            (
                declaration_file_digest(fingerprints, &path, sql_macro),
                Vec::new(),
            )
        };
        let mut definition: NodeDefinition = node(&kind, &name, vec![(DEFINITION_ASPECT, digest)]);
        definition.uses = uses;
        nodes.insert(declaration_key(&identity)?, definition);
    }
    Ok(nodes)
}

fn migrate_from(config_values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let Ok(values) = config_values.cast::<PyDict>() else {
        return Ok(Vec::new());
    };
    let Some(value) = values.get_item(MIGRATE_FROM_KEY)? else {
        return Ok(Vec::new());
    };
    if value.is_instance_of::<PyString>() {
        return Ok(vec![value.extract()?]);
    }
    value.try_iter()?.map(|name| name?.extract()).collect()
}

/// The resolved configuration of a model, defaults and path defaults included, without renames.
fn config_digest(fingerprints: &Fingerprinting, config: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut encoded: Value = fingerprints.canonical(config, &[AUTHORED_KEYS_FIELD])?;
    if let Value::Object(fields) = &mut encoded
        && let Some(Value::Object(values)) = fields.get_mut("values")
    {
        for key in RENAME_CONFIG_KEYS {
            values.remove(key);
        }
    }
    Ok(definition_digest(&[&encoded.to_string()]))
}

/// A model by its authored SQL, resolved configuration, columns, audits and variables it reads.
fn model_node(
    model: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
    uses: &BTreeMap<String, BTreeSet<String>>,
) -> PyResult<(String, NodeDefinition)> {
    let name: String = model_reads::model_name(model)?;
    let config: Bound<'_, PyAny> = model_reads::config(model)?;
    let config_values: Bound<'_, PyAny> = native_field(&config, "values")?;
    let schema: Bound<'_, PyAny> = model_reads::schema_entry(model)?;
    let contract: String = match config_values.cast::<PyDict>() {
        Ok(values) => match values.get_item(CONTRACT_KEY)? {
            Some(value) => fingerprints.canonical(&value, &[])?.to_string(),
            None => String::new(),
        },
        Err(_) => String::new(),
    };
    let (columns, audits): (String, String) = if schema.is_none() {
        (definition_digest(&[&contract]), definition_digest(&[]))
    } else {
        let mut column_audits: Vec<Value> =
            vec![fingerprints.canonical(&native_field(&schema, "audits")?, &[])?];
        for column in native_field(&schema, "columns")?.try_iter()? {
            column_audits.push(fingerprints.canonical(&native_field(&column?, "audits")?, &[])?);
        }
        (
            definition_digest(&[
                &fingerprints.canonical_digest(&schema, (&["name", "tags"], &["audits"]))?,
                &contract,
            ]),
            definition_digest(&[&Value::Array(column_audits).to_string()]),
        )
    };
    let query_sql: String = native_field(model, "authored_query_sql")?.extract()?;
    let authored_sql: String = native_field(model, "authored_sql")?.extract()?;
    let mut aspects: Vec<(&str, String)> = vec![
        (SQL_ASPECT, sql_definition_digest(&query_sql)),
        (CONFIG_ASPECT, config_digest(fingerprints, &config)?),
        (COLUMNS_ASPECT, columns),
        (AUDITS_ASPECT, audits),
    ];
    if let Some(digest) = fingerprints.referenced_vars_digest(&authored_sql) {
        aspects.push((VARS_ASPECT, digest));
    }
    let mut definition: NodeDefinition = node(MODEL_KIND, &name, aspects);
    let key: String = node_key(MODEL_KIND, &name);
    definition.uses = uses.get(&key).into_iter().flatten().cloned().collect();
    definition.uses.extend(project_wide_keys());
    if let Ok(values) = config_values.cast::<PyDict>()
        && let Some(materialized) = values.get_item(MATERIALIZED_KEY)?
        && let Ok(materialized) = materialized.cast::<PyString>()
    {
        definition
            .uses
            .push(node_key(MATERIALIZATION_KIND, materialized.to_str()?));
    }
    definition.migrate_from = migrate_from(&config_values)?;
    Ok((key, definition))
}

/// A source by its declared columns, which shape mocks, and the rest of its declaration.
fn source_node(
    source: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<(String, NodeDefinition)> {
    let name: String = source_reads::name(source)?.extract()?;
    let mut encoded: Value = fingerprints.canonical(&source_reads::source_entry(source)?, &[])?;
    let mut shape: Map<String, Value> = Map::new();
    let mut reads: Option<String> = None;
    if let Value::Object(fields) = &mut encoded {
        for field in SOURCE_SHAPE_FIELDS {
            if let Some(value) = fields.remove(field) {
                shape.insert(field.to_owned(), value);
            }
        }
        if let Some(Value::String(expression)) = fields.get(SOURCE_EXPRESSION_FIELD) {
            reads = fingerprints.referenced_vars_digest(expression);
            let normalised: String = sql_definition_digest(expression);
            fields.insert(SOURCE_EXPRESSION_FIELD.to_owned(), Value::from(normalised));
        }
    }
    let mut aspects: Vec<(&str, String)> = vec![
        (
            DEFINITION_ASPECT,
            definition_digest(&[&encoded.to_string()]),
        ),
        (
            COLUMNS_ASPECT,
            definition_digest(&[&Value::Object(shape).to_string()]),
        ),
    ];
    aspects.extend(reads.map(|digest| (VARS_ASPECT, digest)));
    let mut definition: NodeDefinition = node(SOURCE_KIND, &name, aspects);
    definition.uses = project_wide_keys();
    Ok((node_key(SOURCE_KIND, &name), definition))
}

/// A seed by its data and settings, and by the declared columns that shape its mocks.
fn seed_node(
    seed: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<(String, NodeDefinition)> {
    let name: String = seed_reads::name(seed)?.extract()?;
    let (version_hash, _) = seed_identity(seed)?;
    let schema: Bound<'_, PyAny> = seed_reads::schema_entry(seed)?;
    let columns: String = if schema.is_none() {
        definition_digest(&[])
    } else {
        definition_digest(&[&fingerprints
            .canonical(&native_field(&schema, "columns")?, &[])?
            .to_string()])
    };
    Ok((
        node_key(SEED_KIND, &name),
        node(
            SEED_KIND,
            &name,
            vec![(DEFINITION_ASPECT, version_hash), (COLUMNS_ASPECT, columns)],
        ),
    ))
}

/// Keys of the project-wide nodes that every compiled definition depends on.
pub(crate) fn project_wide_keys() -> Vec<String> {
    [SETTINGS_KIND, CONNECTION_KIND, DEPENDENCIES_KIND]
        .iter()
        .map(|kind| node_key(kind, PROJECT_NODE_NAME))
        .collect()
}

/// The adapter, connections and targets as configured, the selected target, and adapter modules.
fn connection_node(
    project: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<(String, NodeDefinition)> {
    let mut parts: Vec<String> = vec![
        fingerprints.canonical_digest(
            &native_field(discovered, "project_config")?,
            (&PROJECT_CONFIG_ELSEWHERE, &[]),
        )?,
        fingerprints.canonical_digest(
            &native_field(discovered, "local_config")?,
            (&LOCAL_CONFIG_ELSEWHERE, &[]),
        )?,
        text_of(&native_field(project, "effective_target_name")?)?,
    ];
    let adapter_file: Bound<'_, PyAny> = native_field(discovered, "adapter_file")?;
    if !adapter_file.is_none() {
        let path: PathBuf = native_field(&adapter_file, "file_path")?.extract()?;
        parts.push(fingerprints.python_module_digest(&path));
    }
    for module in python_files(&fingerprints.project_dir.join(ADAPTERS_DIRECTORY))? {
        parts.push(fingerprints.python_module_digest(&module));
    }
    let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
    Ok((
        node_key(CONNECTION_KIND, PROJECT_NODE_NAME),
        node(
            CONNECTION_KIND,
            PROJECT_NODE_NAME,
            vec![(DEFINITION_ASPECT, definition_digest(&borrowed))],
        ),
    ))
}

/// The `.py` files directly in `directory`, sorted; none when it does not exist.
fn python_files(directory: &Path) -> PyResult<Vec<PathBuf>> {
    let entries: std::fs::ReadDir = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let path: PathBuf = entry?.path();
        if path.is_file() && path.to_string_lossy().ends_with(PYTHON_SUFFIX) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Loaders, tasks, assets, checks and materializations by their Python modules and imports.
fn python_nodes(
    discovered: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<Vec<(String, NodeDefinition)>> {
    let mut nodes: Vec<(String, NodeDefinition)> = Vec::new();
    let collections = PYTHON_NODE_COLLECTIONS
        .iter()
        .zip(PYTHON_NODE_KINDS)
        .map(|(collection, kind)| (*collection, kind))
        .chain([("materialization_files", MATERIALIZATION_KIND)]);
    for (collection, kind) in collections {
        for item in native_field(discovered, collection)?.try_iter()? {
            let item: Bound<'_, PyAny> = item?;
            let name: String = native_field(&item, "name")?.extract()?;
            let path: PathBuf = native_field(&item, "file_path")?.extract()?;
            nodes.push((
                node_key(kind, &name),
                node(
                    kind,
                    &name,
                    vec![(DEFINITION_ASPECT, fingerprints.python_module_digest(&path))],
                ),
            ));
        }
    }
    Ok(nodes)
}

/// User-defined and table functions by their definitions, Python ones with their imports.
fn function_nodes(
    project: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<Vec<(String, NodeDefinition)>> {
    let mut nodes: Vec<(String, NodeDefinition)> = Vec::new();
    let functions: Bound<'_, PyAny> = project_reads::functions(project)?;
    let hashes: Bound<'_, PyDict> = function_local_hashes(&functions, None)?;
    for function in functions.try_iter()? {
        let function: Bound<'_, PyAny> = function?;
        let key: Bound<'_, PyAny> = function_reads::key(&function)?;
        let kind: String = text_of(&native_field(
            &native_field(&key, "resource_type")?,
            "value",
        )?)?;
        let name: String = function_reads::name(&function)?.extract()?;
        let Some(hash) = hashes.get_item(&name)? else {
            continue;
        };
        let mut digest: String = hash.extract()?;
        let source_file: Bound<'_, PyAny> = function_reads::source_file_path(&function)?;
        if !source_file.is_none() {
            let path: String = text_of(&source_file)?;
            if path.ends_with(PYTHON_SUFFIX) {
                digest = definition_digest(&[
                    &digest,
                    &fingerprints.python_module_digest(Path::new(&path)),
                ]);
            }
        }
        let mut definition: NodeDefinition = node(&kind, &name, vec![(DEFINITION_ASPECT, digest)]);
        for dependency in function_reads::deps(&function)?.try_iter()? {
            let dependency: Bound<'_, PyAny> = dependency?;
            definition.uses.push(node_key(
                &text_of(&native_field(
                    &native_field(&dependency, "resource_type")?,
                    "value",
                )?)?,
                &native_field(&dependency, "name")?.extract::<String>()?,
            ));
        }
        definition.uses.extend(project_wide_keys());
        nodes.push((node_key(&kind, &name), definition));
    }
    Ok(nodes)
}

/// Each unit test file by its text and the variables it reads.
fn test_file_nodes(
    project: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
) -> PyResult<BTreeMap<String, NodeDefinition>> {
    let mut nodes: BTreeMap<String, NodeDefinition> = BTreeMap::new();
    for test in project_reads::sql_tests(project)?.try_iter()? {
        let file: Bound<'_, PyAny> = native_field(&test?, "test_file")?;
        let path: String = text_of(&native_field(&file, "relative_path")?)?;
        let key: String = node_key(TEST_FILE_KIND, &path);
        if nodes.contains_key(&key) {
            continue;
        }
        let contents: String = native_field(&file, "contents")?.extract()?;
        let mut aspects: Vec<(&str, String)> = vec![(
            FILE_ASPECT,
            file_digest(&fingerprints.project_dir.join(&path)),
        )];
        if let Some(digest) = fingerprints.referenced_vars_digest(&contents) {
            aspects.push((VARS_ASPECT, digest));
        }
        nodes.insert(key, node(TEST_FILE_KIND, &path, aspects));
    }
    Ok(nodes)
}

/// Each scenario by its file, captured snapshots and the variables it reads.
fn scenario_nodes(
    project: &Bound<'_, PyAny>,
    fingerprints: &Fingerprinting,
    uses: &BTreeMap<String, BTreeSet<String>>,
) -> PyResult<Vec<(String, NodeDefinition)>> {
    let mut nodes: Vec<(String, NodeDefinition)> = Vec::new();
    let snapshot_root: PathBuf = SCENARIO_SNAPSHOT_ROOT
        .iter()
        .fold(fingerprints.project_dir.clone(), |path, part| {
            path.join(part)
        });
    for scenario in native_field(project, "sql_scenarios")?.try_iter()? {
        let scenario: Bound<'_, PyAny> = scenario?;
        let name: String = native_field(&scenario, "name")?.extract()?;
        let path: String = text_of(&native_field(
            &native_field(&scenario, "scenario_file")?,
            "relative_path",
        )?)?;
        let file: PathBuf = fingerprints.project_dir.join(&path);
        let mut aspects: Vec<(&str, String)> = vec![(FILE_ASPECT, file_digest(&file))];
        if let Some(digest) = directory_digest(&snapshot_root.join(&name)) {
            aspects.push((SNAPSHOTS_ASPECT, digest));
        }
        let contents: String = match std::fs::read_to_string(&file) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        if let Some(digest) = fingerprints.referenced_vars_digest(&contents) {
            aspects.push((VARS_ASPECT, digest));
        }
        let key: String = node_key(SCENARIO_KIND, &name);
        let mut definition: NodeDefinition = node(SCENARIO_KIND, &name, aspects);
        definition.uses = uses.get(&key).into_iter().flatten().cloned().collect();
        nodes.push((key, definition));
    }
    Ok(nodes)
}

/// The definition snapshot of `project`, discovered as `discovered` under `project_dir`.
pub(crate) fn definition_snapshot(
    project: &Bound<'_, PyAny>,
    discovered: &Bound<'_, PyAny>,
    project_dir: &Path,
) -> PyResult<DefinitionSnapshot> {
    let fingerprints: Fingerprinting = Fingerprinting::new(
        project,
        project_dir,
        &native_field(project, "effective_vars")?,
    )?;
    let uses: BTreeMap<String, BTreeSet<String>> = declaration_uses(project)?;
    let mut nodes: BTreeMap<String, NodeDefinition> = declaration_nodes(project, &fingerprints)?;
    nodes.insert(
        node_key(SETTINGS_KIND, PROJECT_NODE_NAME),
        node(
            SETTINGS_KIND,
            PROJECT_NODE_NAME,
            vec![(
                DEFINITION_ASPECT,
                fingerprints.canonical_digest(
                    &native_field(project, "settings")?,
                    (&RUNTIME_SETTINGS, &[]),
                )?,
            )],
        ),
    );
    let (connection_key, connection) = connection_node(project, discovered, &fingerprints)?;
    nodes.insert(connection_key, connection);
    for model in project_reads::models(project)?.try_iter()? {
        let (key, definition) = model_node(&model?, &fingerprints, &uses)?;
        nodes.insert(key, definition);
    }
    for source in project_reads::sources(project)?.try_iter()? {
        let (key, definition) = source_node(&source?, &fingerprints)?;
        nodes.insert(key, definition);
    }
    for seed in project_reads::seeds(project)?.try_iter()? {
        let (key, definition) = seed_node(&seed?, &fingerprints)?;
        nodes.insert(key, definition);
    }
    nodes.extend(function_nodes(project, &fingerprints)?);
    nodes.extend(python_nodes(discovered, &fingerprints)?);
    nodes.extend(test_file_nodes(project, &fingerprints)?);
    nodes.extend(scenario_nodes(project, &fingerprints, &uses)?);
    Ok(DefinitionSnapshot {
        format: DEFINITION_SNAPSHOT_FORMAT,
        nodes,
    })
}
