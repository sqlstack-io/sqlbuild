//! Read the compiled project, plan output and hook identities into manifest inputs.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyList, PyString, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::manifest::models::{
    ManifestAuditPlanInput, ManifestColumnInput, ManifestConfigInput, ManifestDynamicColumnInput,
    ManifestHookInput, ManifestMacroDeclarationInput, ManifestMacroInput, ManifestModelInput,
    ManifestModelSchemaInput, ManifestPathInput, ManifestPlanInput, ManifestPythonHookInput,
    ManifestRelationInput, ManifestSeedInput, ManifestSourceInput, ManifestSqlHookInput,
    ManifestSqlTestPlanInput,
};
use sqlbuild_analysis::python_values::models::PythonValue;

use sqlbuild_python_base::python_values::main::python_value::python_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::RecordId;

const DECIMAL_KIND: &str = "decimal";
const MACRO_KIND: &str = "macro";
const DEFAULT_MATERIALIZATION: &str = "view";

fn attribute<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(value, name)
}

fn text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    attribute(value, name)?.extract()
}

fn optional_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    attribute(value, name)?.extract()
}

/// The value as `json.dumps` encodes it.
fn json(value: &Bound<'_, PyAny>) -> PyResult<PythonValue> {
    python_value(value, false)
}

fn enum_value(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let member: Bound<'_, PyAny> = attribute(value, name)?;
    if member.is_none() {
        return Ok(None);
    }
    native_field(&member, "value")?.extract().map(Some)
}

fn items<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    attribute(value, name)?.try_iter()?.collect()
}

fn texts(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<String>> {
    items(value, name)?
        .iter()
        .map(PyAnyMethods::extract)
        .collect()
}

/// `relative_path.as_posix()` of an optional path attribute.
fn optional_posix(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let path: Bound<'_, PyAny> = attribute(value, name)?;
    if path.is_none() {
        return Ok(None);
    }
    path.call_method0("as_posix")?.extract().map(Some)
}

fn object_key(key: &Bound<'_, PyAny>) -> PyResult<ObjectKey> {
    Ok((
        attribute(key, "resource_type")?.str()?.to_string(),
        text(key, "name")?,
    ))
}

fn keys(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<ObjectKey>> {
    items(value, name)?.iter().map(object_key).collect()
}

/// `dict(value)` when `truthy_only` is unset or the value is truthy, as JSON.
fn dict_copy(value: &Bound<'_, PyAny>, truthy_only: bool) -> PyResult<Option<PythonValue>> {
    if value.is_none() || (truthy_only && !value.is_truthy()?) {
        return Ok(None);
    }
    let copied: Bound<'_, PyAny> = value
        .py()
        .import("builtins")?
        .getattr("dict")?
        .call1((value,))?;
    json(&copied).map(Some)
}

fn path(value: &Bound<'_, PyAny>) -> PyResult<ManifestPathInput> {
    Ok(ManifestPathInput {
        text: value.str()?.to_string(),
        parent_parts: attribute(&attribute(value, "parent")?, "parts")?.extract()?,
        stem: text(value, "stem")?,
    })
}

fn relation(value: &Bound<'_, PyAny>) -> PyResult<ManifestRelationInput> {
    Ok(ManifestRelationInput {
        database: optional_text(value, "database")?,
        schema: optional_text(value, "schema")?,
        name: optional_text(value, "name")?,
        qualified_name: optional_text(value, "qualified_name")?,
    })
}

fn column(column: &Bound<'_, PyAny>) -> PyResult<ManifestColumnInput> {
    Ok(ManifestColumnInput {
        name: text(column, "name")?,
        description: optional_text(column, "description")?.filter(|text| !text.is_empty()),
        meta: dict_copy(&attribute(column, "meta")?, true)?,
        data_type: optional_text(column, "type")?,
    })
}

fn columns(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<ManifestColumnInput>> {
    items(value, name)?.iter().map(column).collect()
}

fn hook(
    entry: &Bound<'_, PyAny>,
    classes: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<ManifestHookInput> {
    let (sql_class, python_class) = classes;
    if entry.is_instance(sql_class)? {
        return Ok(ManifestHookInput::Sql {
            statement: text(entry, "statement")?,
            name: optional_text(entry, "name")?,
            relative_path: optional_posix(entry, "relative_path")?,
            definition_sql: optional_text(entry, "definition_sql")?,
            kwargs: json(&attribute(entry, "kwargs")?)?,
            description: optional_text(entry, "description")?,
        });
    }
    if entry.is_instance(python_class)? {
        return Ok(ManifestHookInput::Python {
            name: text(entry, "name")?,
            kwargs: json(&attribute(entry, "kwargs")?)?,
        });
    }
    Ok(ManifestHookInput::Other)
}

/// The entries when the hooks value is a list or tuple.
fn hooks(value: &Bound<'_, PyAny>) -> PyResult<Option<Vec<ManifestHookInput>>> {
    if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
        return Ok(None);
    }
    let sql_class: Bound<'_, PyAny> = record_class(value.py(), RecordId::SqlHookEntry)?;
    let python_class: Bound<'_, PyAny> = record_class(value.py(), RecordId::PythonHookEntry)?;
    value
        .try_iter()?
        .map(|entry| hook(&entry?, (&sql_class, &python_class)))
        .collect::<PyResult<_>>()
        .map(Some)
}

fn tags(value: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    if !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()) {
        return Ok(None);
    }
    value
        .try_iter()?
        .map(|tag| Ok(tag?.str()?.to_string()))
        .collect::<PyResult<_>>()
        .map(Some)
}

fn config_value<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
    default: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    mapping_get_or(values, key, default)
}

fn config(values: &Bound<'_, PyAny>) -> PyResult<ManifestConfigInput> {
    let py: Python<'_> = values.py();
    let get = |key: &str| mapping_get(values, key);
    let materialized: Bound<'_, PyAny> = config_value(
        values,
        "materialized",
        PyString::new(py, DEFAULT_MATERIALIZATION).into_any(),
    )?;
    Ok(ManifestConfigInput {
        enabled: json(&config_value(
            values,
            "enabled",
            PyBool::new(py, true).to_owned().into_any(),
        )?)?,
        alias: json(&get("alias")?)?,
        schema: json(&get("schema")?)?,
        database: json(&get("database")?)?,
        tags: tags(&get("tags")?)?,
        materialized: if materialized.is_none() {
            DEFAULT_MATERIALIZATION.to_owned()
        } else {
            materialized.str()?.to_string()
        },
        incremental_strategy: json(&get("incremental_strategy")?)?,
        merge_exclude_columns: json(&get("merge_exclude_columns")?)?,
        full_refresh: json(&get("full_refresh")?)?,
        unique_key: json(&get("unique_key")?)?,
        on_schema_change: json(&config_value(
            values,
            "on_schema_change",
            PyString::new(py, "ignore").into_any(),
        )?)?,
        pre_hooks: hooks(&get("pre_hooks")?)?,
        post_hooks: hooks(&get("post_hooks")?)?,
    })
}

fn dynamic_column(family: &Bound<'_, PyAny>) -> PyResult<ManifestDynamicColumnInput> {
    Ok(ManifestDynamicColumnInput {
        name: json(&attribute(family, "name")?)?,
        pivot_column: json(&attribute(family, "pivot_column")?)?,
        value_column: json(&attribute(family, "value_column")?)?,
        aggregate: json(&attribute(family, "aggregate")?)?,
        data_type: json(&attribute(family, "type")?)?,
        name_pattern: json(&attribute(family, "name_pattern")?)?,
    })
}

fn model_schema(entry: &Bound<'_, PyAny>) -> PyResult<Option<ManifestModelSchemaInput>> {
    if entry.is_none() {
        return Ok(None);
    }
    Ok(Some(ManifestModelSchemaInput {
        description: optional_text(entry, "description")?,
        meta: dict_copy(&attribute(entry, "meta")?, false)?,
        columns: columns(entry, "columns")?,
        dynamic_columns: items(entry, "dynamic_columns")?
            .iter()
            .map(dynamic_column)
            .collect::<PyResult<_>>()?,
    }))
}

fn model(model: &Bound<'_, PyAny>) -> PyResult<ManifestModelInput> {
    Ok(ManifestModelInput {
        name: text(model, "name")?,
        path: path(&attribute(model, "relative_path")?)?,
        destination: relation(&attribute(model, "destination")?)?,
        query_sql: text(model, "query_sql")?,
        config: config(&attribute(&attribute(model, "config")?, "values")?)?,
        schema: model_schema(&attribute(model, "schema_entry")?)?,
        deps: keys(model, "deps")?,
    })
}

fn source(source: &Bound<'_, PyAny>) -> PyResult<ManifestSourceInput> {
    let entry: Bound<'_, PyAny> = attribute(source, "source_entry")?;
    let cursor: Bound<'_, PyAny> = attribute(&entry, "cursor_column")?;
    let unique_key: Bound<'_, PyAny> = attribute(&entry, "unique_key")?;
    Ok(ManifestSourceInput {
        name: text(source, "name")?,
        path: attribute(&attribute(source, "source_file")?, "relative_path")?
            .str()?
            .to_string(),
        entry_name: text(&entry, "name")?,
        database: optional_text(&entry, "database")?,
        schema: optional_text(&entry, "schema")?.filter(|text| !text.is_empty()),
        table: optional_text(&entry, "table")?,
        description: optional_text(&entry, "description")?.filter(|text| !text.is_empty()),
        meta: dict_copy(&attribute(&entry, "meta")?, true)?,
        loader: optional_text(&entry, "loader")?,
        write_strategy: enum_value(&entry, "write_strategy")?,
        cursor_column: if cursor.is_none() {
            None
        } else {
            Some(json(&cursor)?)
        },
        unique_key: if unique_key.is_truthy()? {
            Some(json(&unique_key)?)
        } else {
            None
        },
        columns: columns(&entry, "columns")?,
    })
}

fn seed(seed: &Bound<'_, PyAny>) -> PyResult<ManifestSeedInput> {
    let entry: Bound<'_, PyAny> = attribute(seed, "schema_entry")?;
    Ok(ManifestSeedInput {
        name: text(seed, "name")?,
        path: path(&attribute(&attribute(seed, "seed_file")?, "relative_path")?)?,
        destination: relation(&attribute(seed, "destination")?)?,
        tags: texts(&entry, "tags")?,
        description: optional_text(&entry, "description")?.filter(|text| !text.is_empty()),
        meta: dict_copy(&attribute(&entry, "meta")?, true)?,
        columns: columns(&entry, "columns")?,
    })
}

fn macros(project: &Bound<'_, PyAny>) -> PyResult<Vec<ManifestMacroInput>> {
    let loaded: Bound<'_, PyAny> = attribute(project, "loaded_macros")?;
    let mut macros: Vec<ManifestMacroInput> = Vec::new();
    for (key, loaded_macro) in loaded.downcast::<PyDict>()?.iter() {
        macros.push(ManifestMacroInput {
            key: key.extract()?,
            name: text(&loaded_macro, "name")?,
            relative_path: optional_posix(&loaded_macro, "relative_path")?.unwrap_or_default(),
            raw_source: text(&loaded_macro, "raw_source")?,
        });
    }
    Ok(macros)
}

fn macro_declarations(project: &Bound<'_, PyAny>) -> PyResult<Vec<ManifestMacroDeclarationInput>> {
    let scope_index: Bound<'_, PyAny> = attribute(project, "scope_index")?;
    if scope_index.is_none() {
        return Ok(Vec::new());
    }
    let mut declarations: Vec<ManifestMacroDeclarationInput> = Vec::new();
    for record in items(&scope_index, "declarations")? {
        let identity: Bound<'_, PyAny> = attribute(&record, "identity")?;
        if enum_value(&identity, "kind")?.as_deref() != Some(MACRO_KIND) {
            continue;
        }
        let facts: Bound<'_, PyAny> = attribute(&record, "macro")?;
        declarations.push(ManifestMacroDeclarationInput {
            name: text(&identity, "name")?,
            scope: enum_value(&record, "scope")?.unwrap_or_default(),
            owning_path: optional_text(&record, "owning_path")?,
            dependencies: if facts.is_none() {
                None
            } else {
                Some(
                    items(&facts, "dependencies")?
                        .iter()
                        .map(|dependency| text(dependency, "name"))
                        .collect::<PyResult<_>>()?,
                )
            },
        });
    }
    Ok(declarations)
}

fn sql_hook(hook: &Bound<'_, PyAny>) -> PyResult<ManifestSqlHookInput> {
    Ok(ManifestSqlHookInput {
        name: text(hook, "name")?,
        relative_path: optional_posix(hook, "relative_path")?.unwrap_or_default(),
        sql_body: text(hook, "sql_body")?,
        description: optional_text(hook, "description")?.filter(|text| !text.is_empty()),
    })
}

/// Each discovered Python hook's path and identity hashes.
pub(crate) fn python_hooks(
    project: &Bound<'_, PyAny>,
    identities: &Bound<'_, PyAny>,
) -> PyResult<Vec<ManifestPythonHookInput>> {
    items(project, "hook_functions")?
        .iter()
        .map(|hook| {
            let name: String = text(hook, "name")?;
            let identity: Bound<'_, PyAny> = identities.get_item(&name)?;
            Ok(ManifestPythonHookInput {
                relative_path: optional_posix(hook, "relative_path")?.unwrap_or_default(),
                definition_hash: text(&identity, "definition_hash")?,
                version_hash: text(&identity, "version_hash")?,
                name,
            })
        })
        .collect()
}

fn audit_plan(entry: &Bound<'_, PyAny>) -> PyResult<ManifestAuditPlanInput> {
    Ok(ManifestAuditPlanInput {
        name: text(entry, "name")?,
        resolved_sql: text(entry, "resolved_sql")?,
        attached_target_name: optional_text(entry, "attached_target_name")?,
        attached_column_name: optional_text(entry, "attached_column_name")?,
        scope_deps: keys(entry, "scope_deps")?,
    })
}

fn sql_test_plan(entry: &Bound<'_, PyAny>) -> PyResult<ManifestSqlTestPlanInput> {
    let mut parameters: Vec<(String, PythonValue)> = Vec::new();
    for pair in items(entry, "parameter_values")? {
        let (name, value): (String, Bound<'_, PyAny>) = pair.extract()?;
        let raw: Bound<'_, PyAny> = attribute(&value, "value")?;
        let converted: PythonValue = if enum_value(&value, "kind")?.as_deref() == Some(DECIMAL_KIND)
        {
            PythonValue::String(raw.str()?.to_string())
        } else {
            json(&raw)?
        };
        parameters.push((name, converted));
    }
    Ok(ManifestSqlTestPlanInput {
        key: object_key(&attribute(entry, "key")?)?,
        name: text(entry, "name")?,
        chain: items(entry, "chain")?
            .iter()
            .map(|step| Ok((text(step, "model_name")?, text(step, "resolved_sql")?)))
            .collect::<PyResult<_>>()?,
        scope_deps: keys(entry, "scope_deps")?,
        source_path: optional_posix(entry, "source_path")?,
        block_index: json(&attribute(entry, "block_index")?)?,
        parent_name: json(&attribute(entry, "parent_name")?)?,
        case_name: optional_text(entry, "case_name")?,
        case_index: json(&attribute(entry, "case_index")?)?,
        case_fingerprint: optional_text(entry, "case_fingerprint")?,
        parameter_schema: items(entry, "parameter_schema")?
            .iter()
            .map(|parameter| {
                Ok((
                    text(parameter, "name")?,
                    enum_value(parameter, "value_type")?.unwrap_or_default(),
                    json(&attribute(parameter, "nullable")?)?,
                ))
            })
            .collect::<PyResult<_>>()?,
        parameters,
    })
}

/// The plan entries the manifest reads.
pub(crate) fn plan_input(plan_output: &Bound<'_, PyAny>) -> PyResult<Option<ManifestPlanInput>> {
    if plan_output.is_none() {
        return Ok(None);
    }
    Ok(Some(ManifestPlanInput {
        models: items(plan_output, "model_entries")?
            .iter()
            .map(|entry| Ok((text(entry, "name")?, text(entry, "resolved_sql")?)))
            .collect::<PyResult<_>>()?,
        source_load_names: items(plan_output, "source_load_entries")?
            .iter()
            .map(|entry| text(entry, "name"))
            .collect::<PyResult<_>>()?,
        audits: items(plan_output, "audit_entries")?
            .iter()
            .map(audit_plan)
            .collect::<PyResult<_>>()?,
        sql_tests: items(plan_output, "test_entries")?
            .iter()
            .map(sql_test_plan)
            .collect::<PyResult<_>>()?,
    }))
}

/// The project's models, sources, seeds, macros and hooks.
pub(crate) struct ProjectRows {
    pub(crate) run_id: String,
    pub(crate) sql_analysis_dialect: Option<String>,
    pub(crate) models: Vec<ManifestModelInput>,
    pub(crate) sources: Vec<ManifestSourceInput>,
    pub(crate) seeds: Vec<ManifestSeedInput>,
    pub(crate) macros: Vec<ManifestMacroInput>,
    pub(crate) macro_declarations: Vec<ManifestMacroDeclarationInput>,
    pub(crate) sql_hooks: Vec<ManifestSqlHookInput>,
}

/// Read the compiled project.
pub(crate) fn project_rows(project: &Bound<'_, PyAny>) -> PyResult<ProjectRows> {
    Ok(ProjectRows {
        run_id: text(project, "run_id")?,
        sql_analysis_dialect: optional_text(project, "sql_analysis_dialect")?,
        models: items(project, "models")?
            .iter()
            .map(model)
            .collect::<PyResult<_>>()?,
        sources: items(project, "sources")?
            .iter()
            .map(source)
            .collect::<PyResult<_>>()?,
        seeds: items(project, "seeds")?
            .iter()
            .map(seed)
            .collect::<PyResult<_>>()?,
        macros: macros(project)?,
        macro_declarations: macro_declarations(project)?,
        sql_hooks: items(project, "sql_hook_files")?
            .iter()
            .map(sql_hook)
            .collect::<PyResult<_>>()?,
    })
}
