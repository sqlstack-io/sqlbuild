//! BigQuery's native module: SQLQuack's GoogleSQL parser and the BigQuery semantics profile.

use sqlquack_semantics::dialects::constants::BIGQUERY;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{DataType, Expression};
use sqlquack_syntax_bigquery::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::constants::{
    BIGNUMERIC_DIGITS, BIGQUERY_DIALECT, BIGQUERY_STUB_PREFIX, CAST_TYPE_PREFIX, NUMERIC_DIGITS,
    RANGE_ELEMENT_TYPES, STATEMENTS_CONSTRUCT, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{BigQueryDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;

impl NativeDialect for BigQueryDialect {
    fn name(&self) -> &'static str {
        BIGQUERY_DIALECT
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
        &BIGQUERY
    }

    fn normalize_type(&self, spelling: &str) -> Option<String> {
        let sql: String = format!("{CAST_TYPE_PREFIX}{spelling})");
        let parsed: Parsed = parse_strict(&sql).ok()?;
        let ([Expression::Select(select)], []) =
            (parsed.statements.as_slice(), parsed.gaps.as_slice())
        else {
            return None;
        };
        let [Expression::Cast(cast)] = select.expressions.as_slice() else {
            return None;
        };
        // A spelling that closes the cast early leaves text after it, so it names no one type.
        let whole: bool = cast
            .span
            .0
            .is_some_and(|span| span.end == sql.chars().count());
        whole.then(|| compared_type(&cast.to)).flatten()
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let parsed: Parsed = self.parse(sql)?;
        match (parsed.gaps.first(), parsed.statements.len()) {
            (Some(gap), _) => Err(ParserGap {
                construct: gap.construct.to_owned(),
                message: gap.construct.to_owned(),
                offset: gap.start,
                syntax: false,
            }),
            (None, 1) => Ok(()),
            (None, _) => Err(ParserGap {
                construct: STATEMENTS_CONSTRUCT.to_owned(),
                message: STATEMENTS_CONSTRUCT.to_owned(),
                offset: sql.len(),
                syntax: false,
            }),
        }
    }

    fn stub_prefix(&self) -> &'static str {
        BIGQUERY_STUB_PREFIX
    }
}

/// A BigQuery type in SQLBuild's comparison spelling: upper case, no spaces, parameters kept.
/// The parser has already folded the aliases (INT, BIGINT, BYTEINT, DECIMAL, BIGDECIMAL, BOOLEAN);
/// a name or parameter BigQuery refuses, or a collated string, is `None`.
fn compared_type(data_type: &DataType) -> Option<String> {
    Some(match data_type {
        DataType::BigInt { length: None } => "INT64".to_owned(),
        DataType::Decimal { precision, scale } => {
            let given: Vec<u32> = [*precision, *scale].into_iter().flatten().collect();
            decimal("NUMERIC", &given, NUMERIC_DIGITS)?
        }
        DataType::Double {
            precision: None,
            scale: None,
        } => "FLOAT64".to_owned(),
        DataType::Boolean => "BOOLEAN".to_owned(),
        DataType::String { length } => sized("STRING", *length)?,
        DataType::VarBinary { length } => sized("BYTES", *length)?,
        DataType::Date => "DATE".to_owned(),
        DataType::Time {
            precision: None,
            timezone: false,
        } => "TIME".to_owned(),
        DataType::Timestamp {
            precision: None,
            timezone: false,
        } => "DATETIME".to_owned(),
        DataType::Timestamp {
            precision: None,
            timezone: true,
        } => "TIMESTAMP".to_owned(),
        DataType::Interval {
            unit: None,
            to: None,
        } => "INTERVAL".to_owned(),
        DataType::Json => "JSON".to_owned(),
        DataType::Geography {
            subtype: None,
            srid: None,
        } => "GEOGRAPHY".to_owned(),
        DataType::Array {
            element_type,
            dimension: None,
        } => format!("ARRAY<{}>", compared_type(element_type)?),
        DataType::Struct {
            fields,
            nested: true,
        } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    let spelt: String = compared_type(&field.data_type)?;
                    Some(format!("{}{spelt}", field.name.to_ascii_uppercase()))
                })
                .collect::<Option<_>>()?;
            format!("STRUCT<{}>", fields.join(","))
        }
        DataType::Custom { name } => custom_type(&name.replace(' ', "").to_ascii_uppercase())?,
        _ => return None,
    })
}

/// BIGNUMERIC and RANGE, which the parser keeps by name.
fn custom_type(compact: &str) -> Option<String> {
    if let Some(element) = compact
        .strip_prefix("RANGE<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        return RANGE_ELEMENT_TYPES
            .contains(&element)
            .then(|| compact.to_owned());
    }
    if compact == "BIGNUMERIC" {
        return Some(compact.to_owned());
    }
    let inner: &str = compact
        .strip_prefix("BIGNUMERIC(")
        .and_then(|rest| rest.strip_suffix(')'))?;
    let given: Vec<u32> = inner
        .split(',')
        .map(|digits| digits.parse::<u32>().ok())
        .collect::<Option<_>>()?;
    decimal("BIGNUMERIC", &given, BIGNUMERIC_DIGITS)
}

/// A decimal type with BigQuery's bounds: scale at most `max_scale`, precision from the scale
/// (at least 1) to `max_integer_digits` more than it.
fn decimal(
    base: &str,
    given: &[u32],
    (max_scale, max_integer_digits): (u32, u32),
) -> Option<String> {
    let (precision, scale): (u32, u32) = match *given {
        [] => return Some(base.to_owned()),
        [precision] => (precision, 0),
        [precision, scale] => (precision, scale),
        _ => return None,
    };
    let valid: bool =
        scale <= max_scale && precision >= scale.max(1) && precision <= scale + max_integer_digits;
    let shown: Vec<String> = given.iter().map(u32::to_string).collect();
    valid.then(|| format!("{base}({})", shown.join(",")))
}

/// A STRING or BYTES type, whose length when given is at least 1.
fn sized(base: &str, length: Option<u32>) -> Option<String> {
    match length {
        None => Some(base.to_owned()),
        Some(0) => None,
        Some(length) => Some(format!("{base}({length})")),
    }
}
