//! Model catalog data comes directly from the compiler's retained facts and records.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyResult, Python};
use pyo3::types::PyTupleMethods;
use serde_json::{Map, Value, json};
use sqlbuild_analysis::analysis_route::main::query_expression_ranges::query_expression_ranges;
use sqlbuild_analysis::assembly::analysis_session::models::{LineageFacts, LineageRow};
use sqlbuild_analysis::compiled_project::main::model_facts::model_facts;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;
use sqlbuild_analysis::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use sqlbuild_orchestration::snapshots::models::SnapshotNode;

use crate::bindings::_helpers::attachments::cursor_intrinsics::analysis_cursor_splices;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_project::snapshot::{
    dependency_keys, destination, insert_node,
};
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::model_input_view::ModelInputRecord;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;

/// Each model's snapshot node, and the `columns` document of ranges kept out of node identity.
pub(crate) fn model_nodes(
    py: Python<'_>,
    project: &NativeProjectView,
    owner: &NativeCompiledProject,
    mut nodes: BTreeMap<String, SnapshotNode>,
) -> PyResult<(BTreeMap<String, SnapshotNode>, Map<String, Value>)> {
    let dialect: Option<String> = project.sql_analysis_dialect();
    let mut columns_document: Map<String, Value> = Map::new();
    for model in project.models(py).bind(py).iter() {
        let model = model.cast::<NativeCompiledModel>()?.get();
        let (name, _, _) = model.file_texts(py)?;
        let facts = model_facts(owner.facts(), &name)
            .ok_or_else(|| PyValueError::new_err("snapshot model has no retained facts"))?;
        if facts.config.encoding_error.is_some() {
            return Err(PyValueError::new_err(
                "model config cannot be represented in snapshot metadata",
            ));
        }
        let (mut definition, ranges) = model.read(py, |input, _| {
            let record = input
                .compiled
                .as_ref()
                .ok_or_else(|| PyValueError::new_err("snapshot model has not been assembled"))?;
            let columns: Vec<Value> = record
                .analysis
                .inferred
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|(name, data_type, nullability)| {
                    json!({"name": name, "type": data_type, "nullability": nullability})
                })
                .collect();
            let definition: Value = json!({
                "name": facts.name,
                "path": facts.relative_path,
                "config": facts.config.values,
                "materialization": facts.config.materialized,
                "deps": dependency_keys(&record.deps),
                "columns": columns,
            });
            let lineage: Option<&LineageFacts> = record.analysis.lineage.as_ref();
            let ranges: Map<String, Value> = match (dialect.as_deref(), lineage) {
                (Some(dialect), Some(lineage)) => expression_ranges(dialect, input, facts, lineage),
                _ => Map::new(),
            };
            Ok((definition, ranges))
        })?;
        if !ranges.is_empty() {
            columns_document.insert(format!("model:{}", facts.name), Value::Object(ranges));
        }
        definition["destination"] = model.read_destination(py, |value| Ok(destination(value)))?;
        nodes = insert_node(
            nodes,
            format!("model:{}", facts.name),
            SnapshotNode {
                kind: "model".into(),
                definition,
                raw_sql: Some(facts.authored_sql.to_string()),
                compiled_sql: Some(facts.query_sql.to_string()),
                python_source: None,
            },
        )?;
    }
    Ok((nodes, columns_document))
}

/// Each column's `{"expression_range": [start, end]}`, when the analysis SQL rebuilds exactly.
fn expression_ranges(
    dialect: &str,
    input: &ModelInputRecord,
    facts: &CompiledModelFacts,
    lineage: &LineageFacts,
) -> Map<String, Value> {
    let mut ranges: Map<String, Value> = Map::new();
    let (LineageFacts::Native(rows) | LineageFacts::NativeFacts(rows)) = lineage;
    if *input.query_sql != *facts.query_sql || rows.iter().all(|row| row.expression_range.is_none())
    {
        return ranges;
    }
    let Some(placeholders) = placeholder_texts(facts.config.values.get("placeholders")) else {
        return ranges;
    };
    let (typed, typed_splices) = typed_analysis_sql(&input.query_sql, &input.typed_calls, dialect);
    let cursor_type: Option<&str> = facts
        .config
        .values
        .get("cursor_type")
        .and_then(Value::as_str);
    let Ok((analysis_sql, cursor_splices)) = analysis_cursor_splices(&typed, cursor_type) else {
        return ranges;
    };
    let cleaned: Vec<Option<(usize, usize)>> =
        rows.iter().map(|row| row.expression_range).collect();
    let mapped = query_expression_ranges(
        dialect,
        (&analysis_sql, &placeholders),
        &[typed_splices, cursor_splices],
        &cleaned,
    );
    for (row, range) in rows.iter().zip(mapped) {
        let LineageRow { output_column, .. } = row;
        if let (Some((start, end)), false) = (range, output_column.is_empty()) {
            ranges
                .entry(output_column.clone())
                .or_insert_with(|| json!({"expression_range": [start, end]}));
        }
    }
    ranges
}

/// The `placeholders` config as analysis read it, as Python's `str` renders each value, if exact.
fn placeholder_texts(value: Option<&Value>) -> Option<Vec<(String, String)>> {
    let Some(Value::Object(values)) = value else {
        return Some(Vec::new());
    };
    values
        .iter()
        .map(|(name, value)| {
            let text: String = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) if number.is_i64() || number.is_u64() => number.to_string(),
                Value::Bool(true) => "True".to_owned(),
                Value::Bool(false) => "False".to_owned(),
                _ => return None,
            };
            Some((name.clone(), text))
        })
        .collect()
}
