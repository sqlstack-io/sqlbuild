//! Python's dict walk over each parsed statement: tables, CTE names and temporary tables.

use polyglot_sql::{ComplexityGuardOptions, Dialect, Expression, ParseOptions};
use serde_json::{Map, Value, json};
use sqlbuild_core::text::main::python_casefold::python_casefold;

use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::relation_names::constants::{
    CATALOG_KEY, CREATE_TABLE_KEY, CTE_ALIAS_KEY, CTE_LIST_KEY, GENERIC_DIALECT,
    MAX_FUNCTION_CALL_DEPTH, NAME_KEY, QUALIFIER_SEPARATOR, SCHEMA_KEY, TABLE_KEYS, TEMPORARY_KEY,
};
use crate::relation_names::errors::RelationNamesFailure;
use crate::relation_names::models::{ExtractedRelations, RelationName};

/// Python's `extract_relation_names_impl`: None where the wheel's `parse` raises `PolyglotError`.
pub(crate) fn extracted_relations(
    sql: &str,
    dialect: Option<&str>,
) -> Result<Option<ExtractedRelations>, RelationNamesFailure> {
    let dialect: &str = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or(GENERIC_DIALECT);
    let Some(trees) = statement_trees(sql, dialect)? else {
        return Ok(None);
    };
    let mut extracted: ExtractedRelations = ExtractedRelations::default();
    for tree in &trees {
        let nodes: Vec<&Map<String, Value>> = walked(tree);
        let cte_names: Vec<String> = nodes.iter().flat_map(|node| cte_names(node)).collect();
        extracted
            .temporary
            .extend(nodes.iter().filter_map(|node| temporary_name(node)));
        let relations = nodes.iter().filter_map(|node| table_name(node));
        for relation in relations {
            let unqualified: bool = relation.schema.is_none() && relation.database.is_none();
            if unqualified && cte_names.contains(&str::to_lowercase(&relation.name)) {
                continue;
            }
            if !extracted.relations.contains(&relation) {
                extracted.relations.push(relation);
            }
        }
    }
    Ok(Some(extracted))
}

/// Each statement as a tree: SQLQuack's where native, with any gap or error None; else Polyglot's.
fn statement_trees(sql: &str, dialect: &str) -> Result<Option<Vec<Value>>, RelationNamesFailure> {
    let internal = |error: serde_json::Error| RelationNamesFailure::Internal(error.to_string());
    if let AnalysisRoute::Native(native) = analysis_route(Some(dialect)) {
        if sql.trim().is_empty() {
            return Ok(Some(Vec::new()));
        }
        return match native.parse(sql) {
            Ok(parsed) if parsed.gaps.is_empty() => parsed
                .statements
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<Value>, _>>()
                .map(Some)
                .map_err(internal),
            _ => Ok(None),
        };
    }
    let Some(statements) = parsed_statements(sql, dialect)? else {
        return Ok(None);
    };
    statements
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<Value>, _>>()
        .map(Some)
        .map_err(internal)
}

/// The wheel's guarded `parse`: Ok(None) is the `PolyglotError` Python catches.
fn parsed_statements(
    sql: &str,
    dialect: &str,
) -> Result<Option<Vec<Expression>>, RelationNamesFailure> {
    let parser: Dialect = Dialect::get_by_name(dialect).ok_or_else(|| {
        RelationNamesFailure::UnknownDialect(format!("Unknown dialect: {dialect}"))
    })?;
    let guard: ComplexityGuardOptions =
        serde_json::from_value(json!({"maxFunctionCallDepth": MAX_FUNCTION_CALL_DEPTH}))
            .map_err(|error| RelationNamesFailure::Internal(error.to_string()))?;
    let options: ParseOptions = ParseOptions {
        complexity_guard: Some(guard),
    };
    match parser.parse_with_options(sql, &options) {
        Ok(statements) => Ok(Some(statements)),
        Err(_polyglot_error) => Ok(None),
    }
}

/// Python's `_walk`: every object, popping the last pending value first.
fn walked(tree: &Value) -> Vec<&Map<String, Value>> {
    let mut nodes: Vec<&Map<String, Value>> = Vec::new();
    let mut pending: Vec<&Value> = vec![tree];
    while let Some(current) = pending.pop() {
        match current {
            Value::Object(map) => {
                nodes.push(map);
                pending.extend(map.values());
            }
            Value::Array(items) => pending.extend(items.iter()),
            _ => {}
        }
    }
    nodes
}

/// Python's `_identifier`: the `name` string of an object.
fn identifier(value: Option<&Value>) -> Option<&str> {
    value?.as_object()?.get(NAME_KEY)?.as_str()
}

/// Python's `_table_names` for one node.
fn table_name(node: &Map<String, Value>) -> Option<RelationName> {
    if !TABLE_KEYS.iter().all(|key| node.contains_key(*key)) {
        return None;
    }
    let name: &str = identifier(node.get(NAME_KEY))?;
    let schema: Option<&str> = identifier(node.get(SCHEMA_KEY));
    let database: Option<&str> = identifier(node.get(CATALOG_KEY));
    if schema.is_none() && database.is_none() && name.contains(QUALIFIER_SEPARATOR) {
        let parts: Vec<&str> = name.split(QUALIFIER_SEPARATOR).collect();
        return Some(RelationName {
            name: parts[parts.len() - 1].to_owned(),
            schema: Some(parts[parts.len() - 2].to_owned()),
            database: parts
                .len()
                .checked_sub(3)
                .map(|index| parts[index].to_owned()),
        });
    }
    Some(RelationName {
        name: name.to_owned(),
        schema: schema.map(str::to_owned),
        database: database.map(str::to_owned),
    })
}

/// Python's `_cte_names` for one node, casefolded.
fn cte_names(node: &Map<String, Value>) -> Vec<String> {
    let Some(Value::Array(ctes)) = node.get(CTE_LIST_KEY) else {
        return Vec::new();
    };
    ctes.iter()
        .filter_map(|cte| identifier(cte.as_object()?.get(CTE_ALIAS_KEY)))
        .map(str::to_lowercase)
        .collect()
}

/// Python's `_temporary_table_names` for one node, casefolded.
fn temporary_name(node: &Map<String, Value>) -> Option<String> {
    let create: &Map<String, Value> = node.get(CREATE_TABLE_KEY)?.as_object()?;
    if create.get(TEMPORARY_KEY) != Some(&Value::Bool(true)) {
        return None;
    }
    let table: &Map<String, Value> = create.get(NAME_KEY)?.as_object()?;
    identifier(table.get(NAME_KEY)).map(python_casefold)
}
