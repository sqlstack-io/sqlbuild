//! Databricks' native module: SQLQuack's Databricks SQL parser and the Databricks semantics profile.

use sqlquack_semantics::dialects::constants::DATABRICKS;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{DataType, Expression};
use sqlquack_syntax_databricks::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::_helpers::dialects::type_spelling::family_session_type;
use crate::analysis_route::constants::{
    DATABRICKS_BOOLEAN_TYPE, DATABRICKS_CAST_TYPE_PREFIX, DATABRICKS_DATE_TYPE,
    DATABRICKS_DEFAULT_DECIMAL, DATABRICKS_DIALECT, DATABRICKS_STRING_TYPE, UNQUOTED_STUB_PREFIX,
    UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{DatabricksDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect, TypeFamily};

impl NativeDialect for DatabricksDialect {
    fn name(&self) -> &'static str {
        DATABRICKS_DIALECT
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|failure| match failure {
            StrictFailure::Parse(error) => ParserGap {
                construct: error.message.clone(),
                message: error.message,
                offset: error.position,
                syntax: error.syntax,
            },
            StrictFailure::Uncovered { position } => ParserGap {
                construct: UNSCANNED_CONSTRUCT.to_owned(),
                message: UNSCANNED_CONSTRUCT.to_owned(),
                offset: position,
                syntax: true,
            },
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &DATABRICKS
    }

    /// A spelling Databricks cannot read takes its family's type where the family has only one.
    fn session_type(&self, spelling: &str) -> String {
        family_session_type(
            self,
            spelling,
            &[
                (TypeFamily::String, DATABRICKS_STRING_TYPE),
                (TypeFamily::Boolean, DATABRICKS_BOOLEAN_TYPE),
                (TypeFamily::Date, DATABRICKS_DATE_TYPE),
            ],
        )
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        let spelt: String = canonical(&cast_target(spelling)?)?;
        Some(normalize_text_type(&spelt, Some(TypeDialect::Databricks)))
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }

    fn same_type(&self, declared: &NormalizedType, inferred: &NormalizedType) -> bool {
        declared == inferred
            || overlays_string(declared, inferred)
            || overlays_string(inferred, declared)
    }
}

/// The type `spelling` names when Databricks' parser reads it whole as a cast target.
fn cast_target(spelling: &str) -> Option<DataType> {
    let mut depth: usize = 0;
    for character in spelling.chars() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            ';' => return None,
            _ => {}
        }
    }
    if depth != 0 || spelling.trim().is_empty() {
        return None;
    }
    let parsed: Parsed = match parse_strict(&format!("{DATABRICKS_CAST_TYPE_PREFIX}{spelling})")) {
        Ok(parsed) => parsed,
        Err(StrictFailure::Parse(_) | StrictFailure::Uncovered { .. }) => return None,
    };
    let ([Expression::Select(select)], []) = (parsed.statements.as_slice(), parsed.gaps.as_slice())
    else {
        return None;
    };
    match (select.expressions.as_slice(), &select.from) {
        ([Expression::Cast(cast)], None) => Some(cast.to.clone()),
        _ => None,
    }
}

/// One spelling for every way of writing a Databricks type, in Polyglot's names where they agree.
fn canonical(data_type: &DataType) -> Option<String> {
    Some(match data_type {
        DataType::Array { element_type, .. } => format!("ARRAY<{}>", canonical(element_type)?),
        DataType::Map {
            key_type,
            value_type,
        } => format!("MAP<{}, {}>", canonical(key_type)?, canonical(value_type)?),
        DataType::Struct { fields, .. } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    Some(format!(
                        "{}: {}",
                        field_name(&field.name),
                        canonical(&field.data_type)?
                    ))
                })
                .collect::<Option<_>>()?;
            format!("STRUCT<{}>", fields.join(", "))
        }
        DataType::Char {
            length: Some(length),
        } => format!("CHAR({length})"),
        DataType::VarChar {
            length: Some(length),
            ..
        } => format!("VARCHAR({length})"),
        DataType::String { .. } | DataType::Char { .. } | DataType::VarChar { .. } => {
            DATABRICKS_STRING_TYPE.to_owned()
        }
        DataType::Decimal { precision, scale } => {
            let (default_precision, default_scale) = DATABRICKS_DEFAULT_DECIMAL;
            format!(
                "DECIMAL({},{})",
                precision.unwrap_or(default_precision),
                scale.unwrap_or(default_scale)
            )
        }
        DataType::Timestamp { timezone, .. } => match timezone {
            true => "TIMESTAMP",
            false => "TIMESTAMPNTZ",
        }
        .to_owned(),
        DataType::Time { .. } => "TIME".to_owned(),
        DataType::Interval { unit, to } => match (unit, to) {
            (Some(unit), Some(to)) => format!("INTERVAL {unit} TO {to}"),
            (Some(unit), None) => format!("INTERVAL {unit}"),
            _ => "INTERVAL".to_owned(),
        },
        DataType::Boolean => "BOOLEAN".to_owned(),
        DataType::TinyInt { .. } => "TINYINT".to_owned(),
        DataType::SmallInt { .. } => "SMALLINT".to_owned(),
        DataType::Int { .. } => "INT".to_owned(),
        DataType::BigInt { .. } => "BIGINT".to_owned(),
        DataType::Float { .. } => "FLOAT".to_owned(),
        DataType::Double { .. } => "DOUBLE".to_owned(),
        DataType::Date => "DATE".to_owned(),
        DataType::Binary { .. } => "BINARY".to_owned(),
        DataType::Geography { srid, .. } => spatial("GEOGRAPHY", *srid),
        DataType::Geometry { srid, .. } => spatial("GEOMETRY", *srid),
        DataType::Custom { name } => name.to_ascii_uppercase(),
        _ => return None,
    })
}

/// A STRUCT field's name, backquoted unless it is one word.
fn field_name(name: &str) -> String {
    match name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        true => name.to_owned(),
        false => format!("`{}`", name.replace('`', "``")),
    }
}

/// A GEOGRAPHY or GEOMETRY type with its SRID, or `ANY`.
fn spatial(base: &str, srid: Option<u32>) -> String {
    match srid {
        Some(srid) => format!("{base}({srid})"),
        None => format!("{base}(ANY)"),
    }
}

/// Whether `sized` is CHAR(n) or VARCHAR(n), a length check on STRING, and `other` is STRING.
fn overlays_string(sized: &NormalizedType, other: &NormalizedType) -> bool {
    sized.family == TypeFamily::String
        && sized.length.is_some()
        && other.normalized_name == DATABRICKS_STRING_TYPE
}
