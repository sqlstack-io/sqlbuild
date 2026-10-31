//! Sources, functions and audits are read from their retained native records.

use std::collections::BTreeMap;

use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use serde_json::{Value, json};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::snapshot::{
    dependency_keys, destination, insert_node,
};
use crate::bindings::_helpers::compiled_project::snapshot_boundary::{
    boundary_nodes, columns, metadata, relative_path,
};
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::compiled_views::function_views::NativeCompiledFunction;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;

pub(crate) fn resource_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<BTreeMap<String, SnapshotNode>> {
    for source in owner.sources() {
        let attached = &source.attached;
        nodes = insert_node(
            nodes,
            format!("source:{}", attached.name),
            SnapshotNode {
                kind: "source".into(),
                definition: json!({
                    "name": attached.name,
                    "path": relative_path(attached.host.source_file.bind(py))?,
                    "database": attached.database,
                    "schema": attached.schema,
                    "table": attached.table,
                    "expression": attached.expression,
                    "contract": attached.contract,
                    "description": attached.description,
                    "namespace": source.namespace,
                    "columns": columns(attached.host.columns.bind(py).as_any())?,
                    "meta": plain_value(attached.host.meta.bind(py))?,
                    "entry": metadata(attached.host.entry.bind(py), &[
                        "loader", "write_strategy", "cursor_column", "unique_key",
                        "final", "load_batch_size", "freshness",
                    ])?,
                }),
                raw_sql: None,
                compiled_sql: attached.expression.clone(),
                python_source: None,
            },
        )?;
    }
    let function_views = project.functions(py);
    for (function, view) in owner
        .functions()
        .iter()
        .zip(function_views.bind(py).try_iter()?)
    {
        let effective = view?
            .cast::<NativeCompiledFunction>()?
            .get()
            .current_destinations(py)?
            .0;
        let attached = &function.attached;
        let kind = attached.node_type();
        let python = attached.python.as_ref().map(|runtime| {
            json!({
                "version": runtime.runtime_version,
                "entry_point": runtime.entry_point,
                "packages": runtime.packages,
            })
        });
        nodes = insert_node(
            nodes,
            format!("{kind}:{}", attached.name),
            SnapshotNode {
                kind: kind.into(),
                definition: json!({
                    "name": attached.name,
                    "path": attached.relative_path,
                    "arguments": attached.arguments,
                    "returns": attached.returns,
                    "return_columns": attached.return_columns,
                    "destination": destination(&effective),
                    "python": python,
                    "tags": attached.tags,
                    "description": attached.description,
                    "deps": dependency_keys(function.deps.as_deref().unwrap_or_default()),
                }),
                raw_sql: None,
                compiled_sql: attached.python.is_none().then(|| attached.body.clone()),
                python_source: attached.python.is_some().then(|| attached.body.clone()),
            },
        )?;
    }
    for audit in owner.audits() {
        let attached = &audit.attached;
        let target: Option<Value> = attached.target.as_ref().map(
            |target| json!({"kind": target.kind, "name": target.name, "column": target.column}),
        );
        nodes = insert_node(
            nodes,
            format!("audit:{}", attached.compiled_name()),
            SnapshotNode {
                kind: "audit".into(),
                definition: json!({
                    "name": attached.compiled_name(),
                    "definition_name": attached.definition_name,
                    "measurement": attached.measurement,
                    "evidence_sql": attached.evidence_sql,
                    "target": target,
                    "severity": attached.severity,
                    "run_scope": attached.run_scope,
                    "always_run": attached.always_run,
                    "description": attached.description,
                    "policy": metadata(attached.host.audit_block.bind(py), &[
                        "evaluation_mode", "measurement_contract",
                    ])?,
                    "thresholds": plain_value(
                        attached.host.thresholds.bind(py),
                    )?,
                    "minimum_samples": plain_value(
                        attached.host.minimum_samples.bind(py),
                    )?,
                    "evidence_limit": plain_value(
                        attached.host.evidence_limit.bind(py),
                    )?,
                    "deps": dependency_keys(audit.scope_deps.as_deref().unwrap_or_default()),
                }),
                raw_sql: None,
                compiled_sql: Some(attached.sql_body.clone()),
                python_source: None,
            },
        )?;
    }
    boundary_nodes(py, project, owner, nodes)
}
