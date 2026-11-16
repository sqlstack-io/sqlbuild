//! Compile project roles.

use crate::graph::models::ProjectSelection;
use crate::project_snapshot::_helpers::snapshot::snapshot_roles::ROLES_FILE;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::authorization::main::compile_roles::compile_roles;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use std::path::Path;

/// Compile project roles.
pub fn compile_project_roles(
    py: Python<'_>,
    mut input: SnapshotInput,
    (project_dir, graph, selected): (&Path, &ProjectGraph, &ProjectSelection),
) -> PyResult<SnapshotInput> {
    let path = project_dir.join(ROLES_FILE);
    if !path.is_file() {
        return Ok(input);
    }
    if input.documents.contains_key("roles") {
        return Err(PyValueError::new_err(
            "roles.toml cannot overwrite a snapshot roles document",
        ));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| PyValueError::new_err(format!("roles.toml: {error}")))?;
    let (document, warnings) = compile_roles(&text, |selector| {
        let tokens: Vec<String> = selector.split_whitespace().map(str::to_owned).collect();
        selected.node_keys(graph, &tokens, &[])
    })
    .map_err(PyValueError::new_err)?;
    for warning in warnings {
        write_console(py, "stderr", &format!("Warning: {warning}\n"))?;
    }
    input.documents.insert("roles".into(), document);
    Ok(input)
}
