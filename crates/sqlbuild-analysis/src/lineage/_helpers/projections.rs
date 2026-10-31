//! Python's fast lineage reads of wheel expression accessors, over the same crate expressions.

use std::collections::{HashMap, HashSet};

use polyglot_sql::{Expression, ExpressionWalk, ast_json};
use serde_json::Value;

use crate::lineage::_helpers::python_payload::{PythonValue, python_payload};
use crate::lineage::constants::{
    AGGREGATE_KINDS, CAST_KINDS, KIND_ALIAS, KIND_COLUMN, KIND_SELECT, KIND_TABLE, PAYLOAD_COLUMN,
    PAYLOAD_NAME, PAYLOAD_TABLE, SET_OPERATION_KINDS, SET_OPERATION_SIDES, STAR_COLUMN_NAME,
};
use crate::lineage::models::PhysicalResource;
use crate::lineage::models::{
    LineageConfidence, LineageResourceType, LineageSource, LineageTransformKind,
};

/// A read Python would make differently, or fail on, so the model is deferred to Python.
#[derive(Debug)]
pub(crate) struct UnreadableExpression;

/// Table names and aliases mapped to resources, in Python's dictionary insertion order.
pub(crate) struct AliasMap<'a> {
    entries: Vec<(String, &'a PhysicalResource)>,
}

impl<'a> AliasMap<'a> {
    fn insert(&mut self, key: &str, resource: &'a PhysicalResource) {
        match self.entries.iter_mut().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = resource,
            None => self.entries.push((key.to_owned(), resource)),
        }
    }

    fn get(&self, key: &str) -> Option<&'a PhysicalResource> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, resource)| *resource)
    }

    /// Python's `_single_alias_resource`: the one resource every entry names, by resource name.
    pub(crate) fn single_resource(&self) -> Option<&'a PhysicalResource> {
        let mut values = self.entries.iter().map(|(_, resource)| *resource);
        let first = values.next()?;
        values
            .all(|candidate| candidate.resource_name == first.resource_name)
            .then_some(first)
    }
}

pub(crate) fn is_star(expression: &Expression) -> bool {
    match expression {
        Expression::Star(_) => true,
        Expression::Column(column) => column.name.name == STAR_COLUMN_NAME,
        _ => false,
    }
}

/// `projection.this` for an alias, the projection itself otherwise.
pub(crate) fn unaliased(projection: &Expression) -> Result<&Expression, UnreadableExpression> {
    if projection.variant_name() == KIND_ALIAS {
        return projection.get_this().ok_or(UnreadableExpression);
    }
    Ok(projection)
}

/// Python's `_polyglot_projection_output_name`.
pub(crate) fn projection_output_name(
    projection: &Expression,
    index: usize,
    inferred_names: &[String],
) -> Option<String> {
    let output_name = projection.get_output_name();
    if !output_name.is_empty() && output_name != STAR_COLUMN_NAME {
        return Some(output_name.to_owned());
    }
    let alias = projection.get_alias();
    let alias_or_name = if alias.is_empty() {
        projection.get_name()
    } else {
        alias
    };
    if !alias_or_name.is_empty() && alias_or_name != STAR_COLUMN_NAME {
        return Some(alias_or_name.to_owned());
    }
    inferred_names.get(index).cloned()
}

/// Python's `_polyglot_table_alias_map` over `find_all("table")`.
pub(crate) fn table_alias_map<'a>(
    expression: &Expression,
    physical_resources: &'a [PhysicalResource],
) -> AliasMap<'a> {
    let mut by_name: HashMap<&str, &'a PhysicalResource> = HashMap::new();
    for resource in physical_resources {
        let _ = by_name.insert(resource.physical_name.as_str(), resource);
    }
    for resource in physical_resources {
        let _ = by_name.insert(resource.resource_name.as_str(), resource);
    }
    let mut alias_map = AliasMap {
        entries: Vec::new(),
    };
    for table in expression
        .dfs()
        .skip(1)
        .filter(|node| node.variant_name() == KIND_TABLE)
    {
        let table_name = table.get_name();
        let Some(resource) = by_name.get(table_name) else {
            continue;
        };
        alias_map.insert(table_name, resource);
        let alias = table.get_alias();
        let alias_or_name = if alias.is_empty() { table_name } else { alias };
        if !alias_or_name.is_empty() {
            alias_map.insert(alias_or_name, resource);
        }
    }
    alias_map
}

/// Python's `_polyglot_projection_upstream_columns`.
pub(crate) fn projection_upstream_columns(
    projection: &Expression,
    alias_map: &AliasMap<'_>,
    unqualified_resource: Option<&PhysicalResource>,
) -> Result<(Vec<LineageSource>, LineageConfidence), UnreadableExpression> {
    let mut columns: Vec<LineageSource> = Vec::new();
    let mut seen: HashSet<(LineageResourceType, String, String)> = HashSet::new();
    let mut confidence = LineageConfidence::High;
    for (column_name, table_name) in column_refs(projection)? {
        if column_name.is_empty() {
            continue;
        }
        let resource = if !table_name.is_empty() {
            alias_map.get(&table_name)
        } else if let Some(resource) = unqualified_resource {
            confidence = LineageConfidence::Medium;
            Some(resource)
        } else {
            confidence = LineageConfidence::Unknown;
            None
        };
        let Some(resource) = resource else {
            continue;
        };
        let key = (
            resource.resource_type,
            resource.resource_name.clone(),
            column_name,
        );
        if seen.contains(&key) {
            continue;
        }
        columns.push(LineageSource {
            resource_type: key.0,
            resource_name: key.1.clone(),
            column_name: key.2.clone(),
        });
        let _ = seen.insert(key);
    }
    Ok((columns, confidence))
}

/// Python's `_polyglot_classify_transform`.
pub(crate) fn classify_transform(
    expression: &Expression,
    upstream_columns: &[LineageSource],
) -> LineageTransformKind {
    let kind = expression.variant_name();
    if is_star(expression) {
        return LineageTransformKind::Star;
    }
    if CAST_KINDS.contains(&kind) {
        return LineageTransformKind::Cast;
    }
    if expression
        .dfs()
        .any(|node| AGGREGATE_KINDS.contains(&node.variant_name()))
    {
        return LineageTransformKind::Aggregation;
    }
    if upstream_columns.is_empty() {
        return LineageTransformKind::Constant;
    }
    if kind == KIND_COLUMN && upstream_columns.len() == 1 {
        return LineageTransformKind::Direct;
    }
    LineageTransformKind::Expression
}

/// Python's `_polyglot_set_expression_selects`, serialized once and walked iteratively, left first.
pub(crate) fn set_expression_selects(
    expression: &Expression,
) -> Result<Vec<Expression>, UnreadableExpression> {
    let kind = expression.variant_name();
    if kind == KIND_SELECT {
        return Ok(vec![expression.clone()]);
    }
    let mut selects: Vec<Expression> = Vec::new();
    if !SET_OPERATION_KINDS.contains(&kind) {
        return Ok(selects);
    }
    let payload: Value = serde_json::to_value(expression).map_err(|_| UnreadableExpression)?;
    let mut pending: Vec<&Value> = vec![&payload];
    while let Some(node) = pending.pop() {
        if set_operation_kind(node).is_some() {
            let Some(Value::Object(arguments)) = payload_arguments(node) else {
                continue;
            };
            for side in SET_OPERATION_SIDES.iter().rev() {
                if let Some(child) = arguments.get(*side) {
                    pending.push(child);
                }
            }
            continue;
        }
        let Ok(child) = ast_json::expression_from_value(node.clone()) else {
            continue;
        };
        if child.variant_name() == KIND_SELECT {
            selects.push(child);
        } else if SET_OPERATION_KINDS.contains(&child.variant_name()) {
            return Err(UnreadableExpression);
        }
    }
    Ok(selects)
}

/// The set-operation kind a serialized expression is tagged with, if any.
fn set_operation_kind(value: &Value) -> Option<&'static str> {
    let Value::Object(map) = value else {
        return None;
    };
    let (key, _) = map.iter().next().filter(|_| map.len() == 1)?;
    SET_OPERATION_KINDS
        .iter()
        .copied()
        .find(|kind| *kind == key.as_str())
}

/// The wheel's `args`: the payload under an expression's single variant key.
fn payload_arguments(payload: &Value) -> Option<&Value> {
    match payload {
        Value::Object(map) => map.values().next(),
        _ => None,
    }
}

/// Python's `_polyglot_column_refs_in_expression`: `(column, table)` pairs in payload order.
fn column_refs(expression: &Expression) -> Result<Vec<(String, String)>, UnreadableExpression> {
    let payload: PythonValue = python_payload(expression).map_err(|_| UnreadableExpression)?;
    if expression.variant_name() == KIND_COLUMN {
        let table = payload
            .get(PAYLOAD_COLUMN)
            .and_then(|column| column.get(PAYLOAD_TABLE));
        let table_name = match table.and_then(|table| table.get(PAYLOAD_NAME)) {
            Some(PythonValue::Str(name)) => name.clone(),
            _ => String::new(),
        };
        return Ok(vec![(expression.get_name().to_owned(), table_name)]);
    }
    Ok(payload_column_refs(&payload))
}

/// Python's recursive `visit`, depth first in payload order.
fn payload_column_refs(root: &PythonValue) -> Vec<(String, String)> {
    let mut refs: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<&PythonValue> = vec![root];
    while let Some(node) = pending.pop() {
        let children: Vec<&PythonValue> = match node {
            PythonValue::Dict(entries) => {
                if let Some(column @ PythonValue::Dict(_)) = node.get(PAYLOAD_COLUMN) {
                    let table_name = match column.get(PAYLOAD_TABLE) {
                        Some(table @ PythonValue::Dict(_)) => {
                            name_payload_value(table.get(PAYLOAD_NAME))
                        }
                        _ => String::new(),
                    };
                    refs.push((name_payload_value(column.get(PAYLOAD_NAME)), table_name));
                    continue;
                }
                entries.iter().map(|(_, value)| value).collect()
            }
            PythonValue::List(values) => values.iter().collect(),
            _ => Vec::new(),
        };
        pending.extend(children.into_iter().rev());
    }
    refs
}

/// Python's `_polyglot_name_payload_value`.
fn name_payload_value(payload: Option<&PythonValue>) -> String {
    match payload {
        Some(PythonValue::Str(name)) => name.clone(),
        Some(map @ PythonValue::Dict(_)) => match map.get(PAYLOAD_NAME) {
            Some(PythonValue::Str(name)) => name.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
