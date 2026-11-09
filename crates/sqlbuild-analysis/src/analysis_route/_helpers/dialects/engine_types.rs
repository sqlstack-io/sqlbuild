//! The type each engine stores for a normalized type: its documented synonyms and defaults met.

use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily};

/// DuckDB's integer, float, boolean, timestamp and blob aliases, by the type DuckDB stores.
const DUCKDB_ALIASES: [(&str, TypeFamily, &[&str]); 8] = [
    ("INTEGER", TypeFamily::Integer, &["INT", "INT4", "SIGNED"]),
    ("BIGINT", TypeFamily::Integer, &["INT8", "LONG"]),
    ("SMALLINT", TypeFamily::Integer, &["INT2", "SHORT"]),
    ("TINYINT", TypeFamily::Integer, &["INT1"]),
    ("FLOAT", TypeFamily::Float, &["REAL", "FLOAT4"]),
    ("DOUBLE", TypeFamily::Float, &["FLOAT8"]),
    ("BOOLEAN", TypeFamily::Boolean, &["BOOL", "LOGICAL"]),
    ("BLOB", TypeFamily::Other, &["VARBINARY", "BYTEA", "BINARY"]),
];
/// DuckDB's precision and scale of a DECIMAL written without them.
const DUCKDB_DEFAULT_DECIMAL: (i64, i64) = (18, 3);
/// Snowflake's float names, all one 64-bit type.
const SNOWFLAKE_FLOATS: [&str; 4] = ["FLOAT", "FLOAT4", "FLOAT8", "DOUBLE"];
/// Snowflake's fixed-point types of precision 38 and scale 0 that SQLBuild leaves unnormalized.
const SNOWFLAKE_INTEGER_NAMES: [&str; 1] = ["BYTEINT"];
/// Snowflake's single-character string types, whose default length is 1.
const SNOWFLAKE_CHARACTER_NAMES: [&str; 3] = ["CHAR", "CHARACTER", "NCHAR"];
/// The fractional-second precision Snowflake gives a TIME written without one.
const SNOWFLAKE_TIME_PRECISION: &str = "TIME(9)";

/// The type DuckDB stores for `normalized`: string lengths dropped, aliases and defaults met.
pub(crate) fn duckdb_engine_type(normalized: &NormalizedType) -> NormalizedType {
    let name: &str = &normalized.normalized_name;
    if normalized.family == TypeFamily::String || name == "BPCHAR" {
        return named("VARCHAR", TypeFamily::String);
    }
    if name == "DATETIME" {
        return named("TIMESTAMP", TypeFamily::Timestamp);
    }
    if name.starts_with("DECIMAL") {
        let (precision, scale) = match (&normalized.precision, &normalized.scale) {
            (None, _) => (
                PythonInteger::from(DUCKDB_DEFAULT_DECIMAL.0),
                PythonInteger::from(DUCKDB_DEFAULT_DECIMAL.1),
            ),
            (Some(precision), scale) => (
                precision.clone(),
                scale.clone().unwrap_or_else(|| PythonInteger::from(0)),
            ),
        };
        return decimal(precision, scale);
    }
    DUCKDB_ALIASES
        .iter()
        .find(|(stored, _, aliases)| *stored == name || aliases.contains(&name))
        .map_or_else(
            || normalized.clone(),
            |(stored, family, _)| named(stored, *family),
        )
}

/// The type Snowflake stores for `normalized`: synonyms and default lengths and precisions met.
pub(crate) fn snowflake_engine_type(normalized: &NormalizedType) -> NormalizedType {
    let name: &str = &normalized.normalized_name;
    if SNOWFLAKE_INTEGER_NAMES.contains(&name) {
        return decimal(PythonInteger::from(38), PythonInteger::from(0));
    }
    if normalized.family == TypeFamily::Decimal
        && normalized.scale.is_none()
        && let Some(precision) = &normalized.precision
    {
        return decimal(precision.clone(), PythonInteger::from(0));
    }
    if SNOWFLAKE_FLOATS.contains(&name) {
        return named("FLOAT", TypeFamily::Float);
    }
    if name == "DATETIME" {
        return named("TIMESTAMP_NTZ", TypeFamily::Timestamp);
    }
    if name == "TIME" {
        return named(SNOWFLAKE_TIME_PRECISION, normalized.family);
    }
    let character: bool = SNOWFLAKE_CHARACTER_NAMES
        .iter()
        .any(|character| name.split('(').next() == Some(*character));
    if character || normalized.family == TypeFamily::String {
        let length: PythonInteger = normalized
            .length
            .clone()
            .unwrap_or_else(|| PythonInteger::from(if character { 1 } else { 16_777_216 }));
        return NormalizedType {
            normalized_name: format!("VARCHAR({length})"),
            family: TypeFamily::String,
            precision: None,
            scale: None,
            length: Some(length),
        };
    }
    normalized.clone()
}

fn named(name: &str, family: TypeFamily) -> NormalizedType {
    NormalizedType {
        normalized_name: name.to_owned(),
        family,
        precision: None,
        scale: None,
        length: None,
    }
}

fn decimal(precision: PythonInteger, scale: PythonInteger) -> NormalizedType {
    NormalizedType {
        normalized_name: format!("DECIMAL({precision},{scale})"),
        family: TypeFamily::Decimal,
        precision: Some(precision),
        scale: Some(scale),
        length: None,
    }
}
