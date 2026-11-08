//! Python's `_normalize_with_fallback`: type text normalized without a parser.

use crate::type_system::_helpers::python_text::{
    remove_python_space, split_type_and_params, upper,
};
use crate::type_system::constants::{
    BIGNUMERIC_TYPE_NAME, BOOLEAN_TYPE_NAMES, DATE_TYPE_NAME, DATETIME_TYPE_NAME,
    DECIMAL_TYPE_NAMES, DEFAULT_TEXT_LENGTH, FLOAT_TYPE_NAMES, INTEGER_PRECISION, INTEGER_SCALE,
    INTEGER_TYPE_NAMES, NORMALIZED_LTZ_INPUT_TYPE_NAME, NORMALIZED_NTZ_INPUT_TYPE_NAMES,
    NORMALIZED_TZ_INPUT_TYPE_NAME, SIZED_TEXT_TYPE_NAMES, STRING_TYPE_NAMES, TEXT_TYPE_NAME,
    TIMESTAMP_TYPE_TOKEN, UNBOUNDED_TEXT_TYPE_NAMES, VARCHAR_TYPE_NAME,
};
use crate::type_system::models::{NormalizedType, PythonInteger, TypeDialect, TypeFamily};

/// Normalize type text as Python's `_normalize_with_fallback` does.
pub(crate) fn normalize_with_fallback(
    type_sql: &str,
    dialect: Option<TypeDialect>,
) -> NormalizedType {
    let normalized: String = remove_python_space(str::trim(&upper(type_sql)));
    let (base_type, params) = split_type_and_params(&normalized);
    let first: Option<PythonInteger> = params.first().cloned();
    let second: Option<PythonInteger> = params.get(1).cloned();
    let snowflake: bool = dialect == Some(TypeDialect::Snowflake);
    let bigquery: bool = dialect == Some(TypeDialect::BigQuery);

    if INTEGER_TYPE_NAMES.contains(&base_type) {
        if snowflake {
            return snowflake_integer();
        }
        let name: &str = if bigquery { "INT64" } else { base_type };
        return simple(name, TypeFamily::Integer);
    }
    if DECIMAL_TYPE_NAMES.contains(&base_type) {
        let (mut precision, mut scale) = (first, second);
        if snowflake && precision.is_none() {
            precision = Some(INTEGER_PRECISION.into());
            scale = Some(INTEGER_SCALE.into());
        }
        let base_name: &str = if bigquery {
            if base_type == BIGNUMERIC_TYPE_NAME {
                "BIGNUMERIC"
            } else {
                "NUMERIC"
            }
        } else if snowflake {
            "DECIMAL"
        } else {
            base_type
        };
        let normalized_name: String = match (&precision, &scale) {
            (Some(precision), Some(scale)) => format!("{base_name}({precision},{scale})"),
            (Some(precision), None) => format!("{base_name}({precision})"),
            _ => base_name.to_owned(),
        };
        return NormalizedType {
            normalized_name,
            family: TypeFamily::Decimal,
            precision,
            scale,
            length: None,
        };
    }
    if FLOAT_TYPE_NAMES.contains(&base_type) {
        let name: &str = if bigquery { "FLOAT64" } else { base_type };
        return simple(name, TypeFamily::Float);
    }
    if STRING_TYPE_NAMES.contains(&base_type) {
        let mut length: Option<PythonInteger> = first;
        let base_name: &str = if bigquery {
            "STRING"
        } else if snowflake && base_type == TEXT_TYPE_NAME
            || length.is_some() && SIZED_TEXT_TYPE_NAMES.contains(&base_type)
        {
            VARCHAR_TYPE_NAME
        } else {
            base_type
        };
        let mut normalized_name: String = match &length {
            Some(length) => format!("{base_name}({length})"),
            None => base_name.to_owned(),
        };
        if snowflake && UNBOUNDED_TEXT_TYPE_NAMES.contains(&base_type) {
            let bounded: PythonInteger = python_or_default(length);
            normalized_name = format!("VARCHAR({bounded})");
            length = Some(bounded);
        }
        return NormalizedType {
            normalized_name,
            family: TypeFamily::String,
            precision: None,
            scale: None,
            length,
        };
    }
    if BOOLEAN_TYPE_NAMES.contains(&base_type) {
        let name: &str = if bigquery { "BOOL" } else { base_type };
        return simple(name, TypeFamily::Boolean);
    }
    // The top-level type decides the family, so a MAP or STRUCT holding a timestamp is not one.
    let top_level: &str = base_type
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .next()
        .unwrap_or(base_type);
    if top_level.contains(TIMESTAMP_TYPE_TOKEN) {
        return simple(
            &timestamp_normalized_name(base_type, dialect),
            TypeFamily::Timestamp,
        );
    }
    if base_type == DATE_TYPE_NAME {
        return simple(base_type, TypeFamily::Date);
    }
    if base_type == DATETIME_TYPE_NAME {
        return simple(base_type, TypeFamily::Datetime);
    }
    simple(&normalized, TypeFamily::Other)
}

/// Python's `_timestamp_normalized_name`: Snowflake spells its timestamp aliases one way.
pub(crate) fn timestamp_normalized_name(
    normalized_name: &str,
    dialect: Option<TypeDialect>,
) -> String {
    if dialect != Some(TypeDialect::Snowflake) {
        return normalized_name.to_owned();
    }
    let compact: String = normalized_name.replace('_', "");
    if NORMALIZED_NTZ_INPUT_TYPE_NAMES.contains(&compact.as_str()) {
        return "TIMESTAMP_NTZ".to_owned();
    }
    if compact == NORMALIZED_LTZ_INPUT_TYPE_NAME {
        return "TIMESTAMP_LTZ".to_owned();
    }
    if compact == NORMALIZED_TZ_INPUT_TYPE_NAME {
        return "TIMESTAMP_TZ".to_owned();
    }
    normalized_name.to_owned()
}

/// Snowflake's integers: `DECIMAL(38,0)`.
pub(crate) fn snowflake_integer() -> NormalizedType {
    NormalizedType {
        normalized_name: format!("DECIMAL({INTEGER_PRECISION},{INTEGER_SCALE})"),
        family: TypeFamily::Decimal,
        precision: Some(INTEGER_PRECISION.into()),
        scale: Some(INTEGER_SCALE.into()),
        length: None,
    }
}

/// Python's `length or DEFAULT_TEXT_LENGTH`: a missing or zero length takes the default.
pub(crate) fn python_or_default(length: Option<PythonInteger>) -> PythonInteger {
    match length {
        Some(length) if !length.is_zero() => length,
        _ => DEFAULT_TEXT_LENGTH.into(),
    }
}

pub(crate) fn simple(name: &str, family: TypeFamily) -> NormalizedType {
    NormalizedType {
        normalized_name: name.to_owned(),
        family,
        precision: None,
        scale: None,
        length: None,
    }
}
