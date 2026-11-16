//! Read the compiled project views and the Python node graph into DAG artifact inputs.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::dag_artifact::models::{
    DagAuditInput, DagColumnInput, DagFunctionInput, DagLoaderInput, DagModelInput,
    DagModelSchemaInput, DagProjectInput, DagPythonAssetInput, DagPythonGraphInput,
    DagPythonNodeInput, DagRelationInput, DagScenarioInput, DagSeedInput, DagSourceEntryInput,
    DagSourceInput, DagSqlTestCaseInput, DagSqlTestInput,
};
use sqlbuild_analysis::dag_artifact::types::DagValue;

use sqlbuild_analysis::dag_artifact::types::DagColumnLineage;

use sqlbuild_python_base::python_values::main::python_value::python_value;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const AUDIT_SEVERITY_MODULE: &str = "sqlbuild.compiler.auditing.types";
const DECIMAL_KIND: &str = "decimal";
const ASSET_KIND: &str = "asset";
const CHECK_KIND: &str = "check";
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

/// `str(value.<name>)`.
fn path_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(attribute(value, name)?.str()?.to_string())
}

/// The `.value` of an optional enum attribute.
fn enum_value(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let member: Bound<'_, PyAny> = attribute(value, name)?;
    if member.is_none() {
        return Ok(None);
    }
    native_field(&member, "value")?.extract().map(Some)
}

fn texts(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<String>> {
    attribute(value, name)?
        .try_iter()?
        .map(|item| item?.extract())
        .collect()
}

fn items<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    attribute(value, name)?.try_iter()?.collect()
}

fn object_key(key: &Bound<'_, PyAny>) -> PyResult<ObjectKey> {
    Ok((text(key, "resource_type")?, text(key, "name")?))
}

fn keys(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<ObjectKey>> {
    items(value, name)?.iter().map(object_key).collect()
}

fn column(column: &Bound<'_, PyAny>) -> PyResult<DagColumnInput> {
    Ok(DagColumnInput {
        name: text(column, "name")?,
        data_type: optional_text(column, "type")?,
        nullable: attribute(column, "nullable")?.extract()?,
        description: optional_text(column, "description")?,
        meta: dag_value(&attribute(column, "meta")?)?,
    })
}

fn columns(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<DagColumnInput>> {
    items(value, name)?.iter().map(column).collect()
}

fn named_types(value: &Bound<'_, PyAny>, name: &str) -> PyResult<Vec<(String, Option<String>)>> {
    items(value, name)?
        .iter()
        .map(|item| Ok((text(item, "name")?, optional_text(item, "type")?)))
        .collect()
}

fn relation(value: &Bound<'_, PyAny>) -> PyResult<DagRelationInput> {
    Ok(DagRelationInput {
        database: optional_text(value, "database")?,
        schema: optional_text(value, "schema")?,
        name: optional_text(value, "name")?,
        qualified_name: optional_text(value, "qualified_name")?,
        logical_database: optional_text(value, "logical_database")?,
        logical_schema: optional_text(value, "logical_schema")?,
    })
}

fn source(source: &Bound<'_, PyAny>) -> PyResult<DagSourceInput> {
    let entry: Bound<'_, PyAny> = attribute(source, "source_entry")?;
    Ok(DagSourceInput {
        key: object_key(&attribute(source, "key")?)?,
        name: text(source, "name")?,
        path: path_text(&attribute(source, "source_file")?, "relative_path")?,
        entry: DagSourceEntryInput {
            name: text(&entry, "name")?,
            database: optional_text(&entry, "database")?,
            schema: optional_text(&entry, "schema")?,
            table: optional_text(&entry, "table")?,
            description: optional_text(&entry, "description")?,
            loader: optional_text(&entry, "loader")?,
            meta: dag_value(&attribute(&entry, "meta")?)?,
            columns: columns(&entry, "columns")?,
        },
    })
}

fn loaders(project: &Bound<'_, PyAny>) -> PyResult<Vec<DagLoaderInput>> {
    let loaders: Vec<Bound<'_, PyAny>> = items(project, "loader_functions")?;
    let names: Bound<'_, PyDict> = PyDict::new(project.py());
    for loader in &loaders {
        names.set_item(attribute(loader, "function")?, attribute(loader, "name")?)?;
    }
    loaders
        .iter()
        .map(|loader| {
            let mut dependency_names: Vec<String> = Vec::new();
            for dependency in items(loader, "depends_on")? {
                if let Some(name) = names.get_item(dependency)?
                    && !name.is_none()
                {
                    dependency_names.push(name.extract()?);
                }
            }
            Ok(DagLoaderInput {
                name: text(loader, "name")?,
                path: path_text(loader, "relative_path")?,
                destination: optional_text(loader, "destination")?,
                columns: columns(loader, "columns")?,
                dependency_names,
            })
        })
        .collect()
}

fn seed(seed: &Bound<'_, PyAny>) -> PyResult<DagSeedInput> {
    let entry: Bound<'_, PyAny> = attribute(seed, "schema_entry")?;
    Ok(DagSeedInput {
        key: object_key(&attribute(seed, "key")?)?,
        name: text(seed, "name")?,
        path: path_text(&attribute(seed, "seed_file")?, "relative_path")?,
        destination: relation(&attribute(seed, "destination")?)?,
        description: optional_text(&entry, "description")?,
        tags: texts(&entry, "tags")?,
        meta: dag_value(&attribute(&entry, "meta")?)?,
        columns: columns(&entry, "columns")?,
    })
}

fn function(function: &Bound<'_, PyAny>) -> PyResult<DagFunctionInput> {
    Ok(DagFunctionInput {
        name: text(function, "name")?,
        path: path_text(function, "relative_path")?,
        destination: relation(&attribute(function, "destination")?)?,
        body_sql: text(function, "body_sql")?,
        language: enum_value(function, "language")?.unwrap_or_default(),
        tags: texts(function, "tags")?,
        returns: text(function, "returns")?,
        return_columns: named_types(function, "return_columns")?,
        arguments: items(function, "arguments")?
            .iter()
            .map(|argument| Ok((text(argument, "name")?, text(argument, "type")?)))
            .collect::<PyResult<_>>()?,
    })
}

fn model_schema(entry: &Bound<'_, PyAny>) -> PyResult<Option<DagModelSchemaInput>> {
    if entry.is_none() {
        return Ok(None);
    }
    Ok(Some(DagModelSchemaInput {
        description: optional_text(entry, "description")?,
        tags: texts(entry, "tags")?,
        meta: dag_value(&attribute(entry, "meta")?)?,
        columns: columns(entry, "columns")?,
    }))
}

/// `str()` of each configured tag when the `tags` value is a list or tuple.
fn config_tags(values: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    let tags: Bound<'_, PyAny> = mapping_get_or(values, "tags", PyTuple::empty(values.py()))?;
    if !(tags.is_instance_of::<PyList>() || tags.is_instance_of::<PyTuple>()) {
        return Ok(None);
    }
    tags.try_iter()?
        .map(|tag| Ok(tag?.str()?.to_string()))
        .collect::<PyResult<_>>()
        .map(Some)
}

fn model(model: &Bound<'_, PyAny>) -> PyResult<DagModelInput> {
    let values: Bound<'_, PyAny> = attribute(model, "config")?.getattr("values")?;
    let inferred: Bound<'_, PyAny> = attribute(model, "inferred_columns")?;
    Ok(DagModelInput {
        key: object_key(&attribute(model, "key")?)?,
        name: text(model, "name")?,
        path: path_text(model, "relative_path")?,
        destination: relation(&attribute(model, "destination")?)?,
        authored_sql: text(model, "authored_sql")?,
        schema: model_schema(&attribute(model, "schema_entry")?)?,
        config_tags: config_tags(&values)?,
        materialization: mapping_get_or(&values, "materialized", DEFAULT_MATERIALIZATION)?
            .str()?
            .to_string(),
        inferred_columns: if inferred.is_none() {
            None
        } else {
            Some(named_types(model, "inferred_columns")?)
        },
    })
}

fn sql_test_case(test: &Bound<'_, PyAny>) -> PyResult<Option<DagSqlTestCaseInput>> {
    let Some(case_name) = optional_text(test, "case_name")? else {
        return Ok(None);
    };
    let parameter_schema: Vec<(String, String, bool)> = items(test, "parameter_schema")?
        .iter()
        .map(|parameter| {
            Ok((
                text(parameter, "name")?,
                enum_value(parameter, "value_type")?.unwrap_or_default(),
                attribute(parameter, "nullable")?.extract()?,
            ))
        })
        .collect::<PyResult<_>>()?;
    let mut parameters: Vec<(String, DagValue)> = Vec::new();
    for pair in items(test, "parameter_values")? {
        let (name, value): (String, Bound<'_, PyAny>) = pair.extract()?;
        let raw: Bound<'_, PyAny> = attribute(&value, "value")?;
        let is_decimal: bool = enum_value(&value, "kind")?.as_deref() == Some(DECIMAL_KIND);
        let converted: DagValue = if is_decimal {
            DagValue::String(raw.str()?.to_string())
        } else {
            dag_value(&raw)?
        };
        parameters.push((name, converted));
    }
    Ok(Some(DagSqlTestCaseInput {
        case_name,
        block_index: attribute(test, "block_index")?.extract()?,
        parent_name: optional_text(test, "parent_name")?,
        case_index: attribute(test, "case_index")?.extract()?,
        case_fingerprint: optional_text(test, "case_fingerprint")?,
        parameter_schema,
        parameters,
    }))
}

fn sql_test(test: &Bound<'_, PyAny>) -> PyResult<DagSqlTestInput> {
    let test_file_path: Bound<'_, PyAny> =
        attribute(test, "test_file")?.getattr("relative_path")?;
    let source_path: Bound<'_, PyAny> = attribute(test, "source_path")?;
    let source_path: Bound<'_, PyAny> = if source_path.is_none() {
        test_file_path.clone()
    } else {
        source_path
    };
    Ok(DagSqlTestInput {
        key: object_key(&attribute(test, "key")?)?,
        name: text(test, "name")?,
        path: test_file_path.str()?.to_string(),
        source_path: source_path.call_method0("as_posix")?.extract()?,
        mode: enum_value(test, "mode")?.unwrap_or_default(),
        scope_deps: keys(test, "scope_deps")?,
        case: sql_test_case(test)?,
    })
}

fn audit(audit: &Bound<'_, PyAny>) -> PyResult<DagAuditInput> {
    Ok(DagAuditInput {
        key: object_key(&attribute(audit, "key")?)?,
        name: text(audit, "name")?,
        path: path_text(&attribute(audit, "audit_file")?, "relative_path")?,
        scope_deps: keys(audit, "scope_deps")?,
        attached_target_kind: enum_value(audit, "attached_target_kind")?,
        attached_target_name: optional_text(audit, "attached_target_name")?,
        attached_column_name: optional_text(audit, "attached_column_name")?,
        severity: enum_value(audit, "severity")?,
    })
}

fn scenario(scenario: &Bound<'_, PyAny>) -> PyResult<DagScenarioInput> {
    Ok(DagScenarioInput {
        key: object_key(&attribute(scenario, "key")?)?,
        name: text(scenario, "name")?,
        path: path_text(&attribute(scenario, "scenario_file")?, "relative_path")?,
        expected_model_names: texts(scenario, "expected_model_names")?,
        authored_cte_names: items(scenario, "authored_ctes")?
            .iter()
            .map(|cte| text(cte, "name"))
            .collect::<PyResult<_>>()?,
        assertion_names: texts(scenario, "assertion_names")?,
        fixture_names: [
            texts(scenario, "source_fixture_names")?,
            texts(scenario, "ref_fixture_names")?,
            texts(scenario, "seed_fixture_names")?,
            texts(scenario, "dbt_ref_fixture_names")?,
        ],
    })
}

/// The compiled project's resources and target defaults.
pub(crate) fn project_input(project: &Bound<'_, PyAny>) -> PyResult<DagProjectInput> {
    Ok(DagProjectInput {
        sources: items(project, "sources")?
            .iter()
            .map(source)
            .collect::<PyResult<_>>()?,
        loaders: loaders(project)?,
        seeds: items(project, "seeds")?
            .iter()
            .map(seed)
            .collect::<PyResult<_>>()?,
        functions: items(project, "functions")?
            .iter()
            .map(function)
            .collect::<PyResult<_>>()?,
        models: items(project, "models")?
            .iter()
            .map(model)
            .collect::<PyResult<_>>()?,
        sql_tests: items(project, "sql_tests")?
            .iter()
            .map(sql_test)
            .collect::<PyResult<_>>()?,
        audits: items(project, "audits")?
            .iter()
            .map(audit)
            .collect::<PyResult<_>>()?,
        scenarios: items(project, "sql_scenarios")?
            .iter()
            .map(scenario)
            .collect::<PyResult<_>>()?,
        target_database: optional_text(project, "effective_target_database")?,
        target_schema: optional_text(project, "effective_target_schema")?,
    })
}

fn python_asset(node: &Bound<'_, PyAny>) -> PyResult<Option<DagPythonAssetInput>> {
    let asset: Bound<'_, PyAny> = attribute(node, "asset")?;
    if asset.is_none() {
        return Ok(None);
    }
    let lineage: Bound<'_, PyAny> = attribute(&asset, "column_lineage")?;
    let column_lineage: Option<DagColumnLineage> = if lineage.is_none() {
        None
    } else {
        let mut entries: Vec<(String, Vec<(String, String)>)> = Vec::new();
        for (column, refs) in lineage.downcast::<PyDict>()?.iter() {
            let refs: Vec<(String, String)> = refs
                .try_iter()?
                .map(|reference| {
                    let reference: Bound<'_, PyAny> = reference?;
                    Ok((text(&reference, "node")?, text(&reference, "column")?))
                })
                .collect::<PyResult<_>>()?;
            entries.push((column.extract()?, refs));
        }
        Some(entries)
    };
    Ok(Some(DagPythonAssetInput {
        columns: columns(&asset, "columns")?,
        column_lineage,
    }))
}

fn check_severity(node: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let check: Bound<'_, PyAny> = attribute(node, "check")?;
    if check.is_none() {
        return Ok(None);
    }
    let severity: Bound<'_, PyAny> = native_field(&check, "severity")?.getattr("value")?;
    node.py()
        .import(AUDIT_SEVERITY_MODULE)?
        .getattr("AuditSeverity")?
        .call1((severity,))?
        .getattr("value")?
        .extract()
        .map(Some)
}

fn python_node(node: &Bound<'_, PyAny>) -> PyResult<DagPythonNodeInput> {
    let kind: String = enum_value(node, "kind")?.unwrap_or_default();
    let meta: Bound<'_, PyAny> = attribute(node, "meta")?;
    Ok(DagPythonNodeInput {
        asset: if kind == ASSET_KIND {
            python_asset(node)?
        } else {
            None
        },
        check_severity: if kind == CHECK_KIND {
            check_severity(node)?
        } else {
            None
        },
        kind,
        name: text(node, "name")?,
        path: path_text(node, "relative_path")?,
        description: optional_text(node, "description")?,
        tags: texts(node, "tags")?,
        group: optional_text(node, "group")?,
        meta: if meta.is_truthy()? {
            dag_value(&meta)?
        } else {
            DagValue::Object(Vec::new())
        },
        sql_deps: items(node, "sql_deps")?
            .iter()
            .map(|reference| {
                Ok((
                    enum_value(reference, "kind")?.unwrap_or_default(),
                    text(reference, "name")?,
                ))
            })
            .collect::<PyResult<_>>()?,
    })
}

/// The discovered Python node graph.
pub(crate) fn python_graph_input(graph: &Bound<'_, PyAny>) -> PyResult<DagPythonGraphInput> {
    let mut kinds_by_name: Vec<(String, String)> = Vec::new();
    for (name, node) in attribute(graph, "nodes_by_name")?
        .downcast::<PyDict>()?
        .iter()
    {
        kinds_by_name.push((
            name.extract()?,
            enum_value(&node, "kind")?.unwrap_or_default(),
        ));
    }
    Ok(DagPythonGraphInput {
        nodes: items(graph, "nodes")?
            .iter()
            .map(python_node)
            .collect::<PyResult<_>>()?,
        dependency_edges: items(graph, "dependency_edges")?
            .iter()
            .map(|edge| Ok((text(edge, "upstream_name")?, text(edge, "downstream_name")?)))
            .collect::<PyResult<_>>()?,
        kinds_by_name,
    })
}

/// A value as `asdict` copies it into the artifact.
fn dag_value(value: &Bound<'_, PyAny>) -> PyResult<DagValue> {
    python_value(value, true)
}
