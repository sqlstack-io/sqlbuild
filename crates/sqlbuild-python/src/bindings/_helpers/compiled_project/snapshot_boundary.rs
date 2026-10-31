//! Discovery files and authored test payloads remain the Python host's boundary.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::snapshot::{destination, insert_node};
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::compiled_views::seed_views::{NativeCompiledSeed, SeedViews};
use crate::bindings::_helpers::records::fields::{field, path_posix};

/// Read only declared metadata fields, never callable attributes or connection settings.
pub(crate) fn metadata(value: &Bound<'_, PyAny>, names: &[&str]) -> PyResult<Value> {
    let mut result = Map::new();
    for name in names {
        result.insert((*name).into(), plain_value(&field(value, name)?)?);
    }
    Ok(Value::Object(result))
}

pub(crate) fn relative_path(value: &Bound<'_, PyAny>) -> PyResult<String> {
    path_posix(&field(value, "relative_path")?)
}

/// Source locations are local compiler diagnostics, not relocatable column metadata.
pub(crate) fn columns(value: &Bound<'_, PyAny>) -> PyResult<Vec<Value>> {
    value
        .try_iter()?
        .map(|column| {
            metadata(
                &column?,
                &["name", "type", "nullable", "description", "meta"],
            )
        })
        .collect()
}

fn file_digest(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let path: PathBuf = field(value, "file_path")?.extract()?;
    value.py().detach(|| {
        let mut file = File::open(path)?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let length = file.read(&mut buffer)?;
            if length == 0 {
                return Ok(format!("{:x}", hash.finalize()));
            }
            hash.update(&buffer[..length]);
        }
    })
}

pub(crate) fn boundary_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    let seed_views = project.seeds(py);
    for (seed, view) in owner.seeds().iter().zip(seed_views.bind(py).try_iter()?) {
        let effective = view?
            .cast::<NativeCompiledSeed>()?
            .get()
            .current_destination(py)?;
        let parts = seed.parts();
        let seed_file = parts.seed_file.bind(py);
        nodes = insert_node(
            nodes,
            format!("seed:{}", seed.name),
            SnapshotNode {
                kind: "seed".into(),
                definition: json!({
                    "name": seed.name,
                    "path": relative_path(seed_file)?,
                    "content_hash": file_digest(seed_file)?,
                    "destination": destination(&effective),
                    "schema": metadata(parts.schema_entry.bind(py), &[
                        "tags", "description", "meta", "csv_settings",
                    ])?,
                    "columns": columns(&field(parts.schema_entry.bind(py), "columns")?)?,
                }),
                raw_sql: None,
                compiled_sql: None,
                python_source: None,
            },
        )?;
    }
    for (kind, objects, names) in [
        (
            "sql_test",
            project.sql_tests(py),
            ["mode", "payload", "parameter_schema", "parameter_values"].as_slice(),
        ),
        (
            "sql_scenario",
            project.sql_scenarios(py),
            ["authored_ctes", "expected_ctes", "assertion_ctes"].as_slice(),
        ),
    ] {
        for item in objects.bind(py).try_iter()? {
            let item = item?;
            let key = field(&item, "key")?;
            let name: String = field(&key, "name")?.extract()?;
            let resource_type: String = field(&key, "resource_type")?.str()?.extract()?;
            let definition = metadata(&item, names)?;
            nodes = insert_node(
                nodes,
                format!("{resource_type}:{name}"),
                SnapshotNode {
                    kind: kind.into(),
                    definition: json!({"name": name, "test": definition}),
                    raw_sql: None,
                    compiled_sql: Some(field(&item, "sql_body")?.extract()?),
                    python_source: None,
                },
            )?;
        }
    }
    for (kind, objects) in [
        ("loader", project.loader_functions(py)),
        ("hook", project.hook_functions(py)),
        ("materialization", project.materialization_files(py)),
    ] {
        for item in objects.bind(py).try_iter()? {
            let item = item?;
            let name: String = field(&item, "name")?.extract()?;
            let path: PathBuf = field(&item, "file_path")?.extract()?;
            let source = py.detach(|| std::fs::read_to_string(path))?;
            nodes = insert_node(
                nodes,
                format!("{kind}:{name}"),
                SnapshotNode {
                    kind: kind.into(),
                    definition: json!({"name": name, "path": relative_path(&item)?}),
                    raw_sql: None,
                    compiled_sql: None,
                    python_source: Some(source),
                },
            )?;
        }
    }
    for item in project.sql_hook_files(py).bind(py).try_iter()? {
        let item = item?;
        let name: String = field(&item, "name")?.extract()?;
        nodes = insert_node(
            nodes,
            format!("sql_hook:{name}"),
            SnapshotNode {
                kind: "sql_hook".into(),
                definition: json!({"name": name, "path": relative_path(&item)?}),
                raw_sql: Some(field(&item, "contents")?.extract()?),
                compiled_sql: Some(field(&item, "sql_body")?.extract()?),
                python_source: None,
            },
        )?;
    }
    let macros = project.loaded_macros(py);
    for (key, item) in macros.bind(py).cast::<PyDict>()?.iter() {
        let name: String = key.extract()?;
        nodes = insert_node(
            nodes,
            format!("macro:{name}"),
            SnapshotNode {
                kind: "macro".into(),
                definition: json!({"name": name, "path": relative_path(&item)?}),
                raw_sql: None,
                compiled_sql: None,
                python_source: Some(field(&item, "raw_source")?.extract()?),
            },
        )?;
    }
    Ok(nodes)
}
