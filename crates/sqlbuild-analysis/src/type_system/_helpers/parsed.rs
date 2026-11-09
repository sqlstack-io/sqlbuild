//! A parsed type's normalization: its tag, parameters and dialect spelling.

use serde_json::Value;
use sqlquack_syntax::tokens::models::Dialect;

use crate::type_system::_helpers::fallback::{
    normalize_with_fallback, python_or_default, simple, snowflake_integer,
    timestamp_normalized_name,
};
use crate::type_system::_helpers::python_text::{python_int, split_type_and_params, upper};
use crate::type_system::_helpers::spelling::{SpellingDialect, spell};
use crate::type_system::_helpers::type_reader::read_type;
use crate::type_system::constants::{
    BIGNUMERIC_TYPE_NAME, BIGQUERY_STRING_TYPE_NAME, BOOLEAN_TYPE_NAMES,
    CUSTOM_NORMALIZATION_TYPE_NAMES, CUSTOM_TYPE_TAG, DATE_TYPE_NAME, DATETIME_TYPE_NAME,
    DECIMAL_TYPE_NAMES, FLOAT_TYPE_NAMES, FLOAT_WIRE_TYPE_NAME, INTEGER_PARSE_TYPE_NAMES,
    INTEGER_PRECISION, INTEGER_SCALE, INTEGER_TYPE_NAMES, MAX_TYPE_NESTING, SIZED_TEXT_TYPE_NAMES,
    STRING_TYPE_NAMES, TIMESTAMP_TYPE_NAMES, TYPE_TAG_ALIASES, UNBOUNDED_TEXT_TYPE_NAMES,
    VARCHAR_TYPE_NAME,
};
use crate::type_system::models::{NormalizedType, PythonInteger, TypeDialect, TypeFamily};
use crate::warehouse_types::main::type_tree::type_tree;
use crate::warehouse_types::models::TypeParser;

/// The type tree of `type_sql` from SQLQuack's parser, else SQLBuild's reader, or the error.
pub(crate) fn parse_type(type_sql: &str, dialect: SpellingDialect) -> Result<Value, String> {
    if nesting_bound(type_sql) > MAX_TYPE_NESTING {
        return Err("the type nests too deeply".to_owned());
    }
    let parser: TypeParser = match dialect {
        SpellingDialect::Snowflake => TypeParser::Core(Dialect::Snowflake),
        SpellingDialect::DuckDb => TypeParser::Core(Dialect::DuckDB),
        SpellingDialect::Sqlite => TypeParser::Core(Dialect::SQLite),
        SpellingDialect::Postgres => TypeParser::Postgres,
        SpellingDialect::BigQuery => TypeParser::BigQuery,
        SpellingDialect::Generic
        | SpellingDialect::Databricks
        | SpellingDialect::Tsql
        | SpellingDialect::ClickHouse => return read_type(type_sql, dialect),
    };
    let native: Result<Value, String> = type_tree(type_sql, parser);
    match native {
        Ok(tree) if !has_custom(&tree) => Ok(tree),
        Ok(tree) => Ok(read_type(type_sql, dialect).unwrap_or(tree)),
        Err(error) => read_type(type_sql, dialect).map_err(|_| error),
    }
}

/// A bound on the type tree's depth: the deepest brackets plus the array suffixes.
pub(crate) fn nesting_bound(type_sql: &str) -> usize {
    let (mut depth, mut deepest, mut suffixes): (usize, usize, usize) = (0, 0, 0);
    for character in type_sql.chars() {
        match character {
            '(' | '<' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            ')' | '>' => depth = depth.saturating_sub(1),
            '[' => suffixes += 1,
            _ => {}
        }
    }
    deepest + suffixes
}

/// Whether `tree` or any type nested in it is an unknown type name.
fn has_custom(tree: &Value) -> bool {
    match tree {
        Value::Object(fields) => {
            fields
                .get("data_type")
                .and_then(Value::as_str)
                .is_some_and(|tag| tag.eq_ignore_ascii_case(CUSTOM_TYPE_TAG))
                || fields.values().any(has_custom)
        }
        Value::Array(items) => items.iter().any(has_custom),
        _ => false,
    }
}

/// The normalization of a parsed type.
pub(crate) fn normalize_parsed(
    tree: &Value,
    spelling: SpellingDialect,
    dialect: Option<TypeDialect>,
) -> NormalizedType {
    let normalized_name: String = upper(&spell(tree, spelling)).replace(' ', "");
    let dtype_name: String = type_name(tree);
    let params: Vec<PythonInteger> = type_params(tree);
    let snowflake: bool = dialect == Some(TypeDialect::Snowflake);
    let bigquery: bool = dialect == Some(TypeDialect::BigQuery);
    let dtype: &str = dtype_name.as_str();

    if bigquery && INTEGER_PARSE_TYPE_NAMES.contains(&dtype) {
        return simple("INT64", TypeFamily::Integer);
    }
    if bigquery && dtype == BIGNUMERIC_TYPE_NAME {
        return simple("BIGNUMERIC", TypeFamily::Decimal);
    }
    if bigquery && dtype == FLOAT_WIRE_TYPE_NAME {
        return simple("FLOAT64", TypeFamily::Float);
    }
    if dtype == CUSTOM_TYPE_TAG {
        let raw_name: String = upper(&match tree.get("name").and_then(Value::as_str) {
            Some(name) => name.to_owned(),
            None => normalized_name.clone(),
        })
        .replace(' ', "");
        if snowflake && raw_name.starts_with("NUMBER") {
            return snowflake_number(&raw_name);
        }
        return normalize_with_fallback(&raw_name, dialect);
    }
    if INTEGER_TYPE_NAMES.contains(&dtype) {
        if snowflake {
            return snowflake_integer();
        }
        return simple(&normalized_name, TypeFamily::Integer);
    }
    if DECIMAL_TYPE_NAMES.contains(&dtype) {
        let mut precision: Option<PythonInteger> = params.first().cloned();
        let mut scale: Option<PythonInteger> = params.get(1).cloned();
        let mut name: String = normalized_name;
        if snowflake && precision.is_none() {
            precision = Some(INTEGER_PRECISION.into());
            scale = Some(INTEGER_SCALE.into());
            name = format!("DECIMAL({INTEGER_PRECISION},{INTEGER_SCALE})");
        }
        return NormalizedType {
            normalized_name: name,
            family: TypeFamily::Decimal,
            precision,
            scale,
            length: None,
        };
    }
    if FLOAT_TYPE_NAMES.contains(&dtype) {
        return simple(&normalized_name, TypeFamily::Float);
    }
    if STRING_TYPE_NAMES.contains(&dtype) {
        let mut length: Option<PythonInteger> = params.first().cloned();
        let mut name: String = normalized_name;
        let base_name: &str = name.split('(').next().unwrap_or_default();
        if snowflake && UNBOUNDED_TEXT_TYPE_NAMES.contains(&base_name) {
            let bounded: PythonInteger = python_or_default(length);
            name = format!("VARCHAR({bounded})");
            length = Some(bounded);
        } else if bigquery && UNBOUNDED_TEXT_TYPE_NAMES.contains(&base_name) {
            name = name.replacen(base_name, BIGQUERY_STRING_TYPE_NAME, 1);
        } else if length.is_some() && SIZED_TEXT_TYPE_NAMES.contains(&base_name) {
            name = name.replacen(base_name, VARCHAR_TYPE_NAME, 1);
        }
        return NormalizedType {
            normalized_name: name,
            family: TypeFamily::String,
            precision: None,
            scale: None,
            length,
        };
    }
    if BOOLEAN_TYPE_NAMES.contains(&dtype) {
        return simple(&normalized_name, TypeFamily::Boolean);
    }
    if TIMESTAMP_TYPE_NAMES.contains(&dtype) {
        if bigquery && normalized_name == DATETIME_TYPE_NAME {
            return simple(&normalized_name, TypeFamily::Datetime);
        }
        return simple(
            &timestamp_normalized_name(&normalized_name, dialect),
            TypeFamily::Timestamp,
        );
    }
    if dtype == DATE_TYPE_NAME {
        return simple(&normalized_name, TypeFamily::Date);
    }
    if dtype == DATETIME_TYPE_NAME {
        return simple(&normalized_name, TypeFamily::Datetime);
    }
    simple(&normalized_name, TypeFamily::Other)
}

/// The scalar parameters of a type tree, keyed as the tree names them.
pub(crate) fn type_args(tree: &Value) -> serde_json::Map<String, Value> {
    let mut args: serde_json::Map<String, Value> = serde_json::Map::new();
    for (key, value) in tree.as_object().into_iter().flatten() {
        if !value.is_array() && !value.is_object() {
            args.insert(key.clone(), value.clone());
        }
    }
    args
}

/// Snowflake's `NUMBER(p,s)` spelled as a custom type: Python's `DECIMAL` rewrite.
fn snowflake_number(raw_name: &str) -> NormalizedType {
    let mut decimal_name: String = raw_name.replacen("NUMBER", "DECIMAL", 1);
    let (_, params) = split_type_and_params(&decimal_name);
    let mut precision: Option<PythonInteger> = params.first().cloned();
    let mut scale: Option<PythonInteger> = params.get(1).cloned();
    if precision.is_none() {
        precision = Some(INTEGER_PRECISION.into());
        scale = Some(INTEGER_SCALE.into());
        decimal_name = format!("DECIMAL({INTEGER_PRECISION},{INTEGER_SCALE})");
    }
    NormalizedType {
        normalized_name: decimal_name,
        family: TypeFamily::Decimal,
        precision,
        scale,
        length: None,
    }
}

/// The type's tag, upper-cased with its word separators as the type names spell them.
fn type_name(tree: &Value) -> String {
    let tag: String = upper(tree.get("data_type").and_then(Value::as_str).unwrap_or(""));
    if let Some((_, alias)) = TYPE_TAG_ALIASES.iter().find(|(name, _)| *name == tag) {
        return (*alias).to_owned();
    }
    if tag == CUSTOM_TYPE_TAG {
        let name: String =
            upper(tree.get("name").and_then(Value::as_str).unwrap_or("")).replace(' ', "");
        if CUSTOM_NORMALIZATION_TYPE_NAMES.contains(&name.as_str()) {
            return name;
        }
    }
    tag
}

/// The type's precision, scale and length, in that order, where it has them.
fn type_params(tree: &Value) -> Vec<PythonInteger> {
    ["precision", "scale", "length"]
        .iter()
        .filter_map(|key| tree.get(*key))
        .filter_map(|value| match value {
            Value::Number(number) => python_int(&number.to_string()),
            _ => None,
        })
        .collect()
}
