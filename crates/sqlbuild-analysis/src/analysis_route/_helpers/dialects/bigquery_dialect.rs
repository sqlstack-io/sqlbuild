//! BigQuery's native module: SQLQuack's GoogleSQL parser and the BigQuery semantics profile.

use sqlquack_semantics::dialects::constants::BIGQUERY;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_semantics::lineage::models::{NullabilityRule, SchemaColumn};
use sqlquack_semantics::lineage::types::LineageSchema;
use sqlquack_syntax::{DataType, Expression};
use sqlquack_syntax_bigquery::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::_helpers::analysis::native_analysis::native_analysis;
use crate::analysis_route::_helpers::dialects::type_spelling::family_session_type;
use crate::analysis_route::constants::{
    BIGNUMERIC_DIGITS, BIGQUERY_DIALECT, CAST_TYPE_PREFIX, DECIMAL_RADIX, NUMERIC_DIGITS,
    POLYGLOT_MAX_DROPPED, POLYGLOT_NESTED_TYPE_NAMES, POLYGLOT_TOP_LEVEL_TYPE_NAMES,
    RANGE_ELEMENT_TYPES, RANGE_TYPE_NAME, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{
    AnalysisInput, BigQueryDialect, BigQueryTypes, NativeAnalysis, ParserGap,
};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect, TypeFamily};

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

    /// The query read against a schema that knows BigQuery's own types, which a cast may name.
    fn analyse(&self, input: &AnalysisInput<'_>) -> Result<NativeAnalysis, ParserGap> {
        let statement: Expression = self.read_whole(input.sql)?;
        let types: BigQueryTypes<'_> = BigQueryTypes {
            schema: input.schema,
        };
        Ok(native_analysis(
            self,
            &statement,
            &AnalysisInput {
                schema: &types,
                ..*input
            },
        ))
    }

    /// A spelling BigQuery cannot read, such as TEXT or DOUBLE, takes its family's BigQuery type.
    fn session_type(&self, spelling: &str) -> String {
        family_session_type(
            self,
            spelling,
            &[
                (TypeFamily::String, "STRING"),
                (TypeFamily::Integer, "INT64"),
                (TypeFamily::Float, "FLOAT64"),
                (TypeFamily::Decimal, "NUMERIC"),
                (TypeFamily::Boolean, "BOOL"),
                (TypeFamily::Date, "DATE"),
            ],
        )
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        let sql: String = format!("{CAST_TYPE_PREFIX}{spelling})");
        let parsed: Parsed = match parse_strict(&sql) {
            Ok(parsed) => parsed,
            Err(StrictFailure::Parse(_) | StrictFailure::Uncovered { .. }) => return None,
        };
        let ([Expression::Select(select)], []) =
            (parsed.statements.as_slice(), parsed.gaps.as_slice())
        else {
            return None;
        };
        let [Expression::Cast(cast)] = select.expressions.as_slice() else {
            return None;
        };
        let ends_the_query: bool = cast
            .span
            .0
            .is_some_and(|span| span.end == sql.chars().count());
        (ends_the_query && native_type(&cast.to).is_some())
            .then(|| polyglot_type(spelling, &cast.to))
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }
}

impl LineageSchema for BigQueryTypes<'_> {
    fn columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.schema.columns(relation)
    }

    fn open_columns(&self, relation: &str) -> Option<&[SchemaColumn]> {
        self.schema.open_columns(relation)
    }

    fn function_type(&self, name: &str) -> Option<&str> {
        self.schema.function_type(name)
    }

    fn function_nullability(&self, name: &str) -> Option<NullabilityRule> {
        self.schema.function_nullability(name)
    }

    fn known_function(&self, name: &str) -> bool {
        self.schema.known_function(name)
    }

    fn function_arities(&self, name: &str, canonical: &str) -> Option<Vec<(usize, Option<usize>)>> {
        self.schema.function_arities(name, canonical)
    }

    fn known_type(&self, name: &str) -> bool {
        self.schema.known_type(name) || BigQueryDialect.normalize_type(name).is_some()
    }
}

/// A type BigQuery's dry run accepts, as BigQuery names it; `None` for one it refuses.
fn native_type(data_type: &DataType) -> Option<String> {
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
        } => format!("ARRAY<{}>", native_type(element_type)?),
        DataType::Struct {
            fields,
            nested: true,
        } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    let spelt: String = native_type(&field.data_type)?;
                    Some(format!("{}{spelt}", field.name.to_ascii_uppercase()))
                })
                .collect::<Option<_>>()?;
            format!("STRUCT<{}>", fields.join(","))
        }
        DataType::Custom { name } => custom_type(&name.replace(' ', "").to_ascii_uppercase())?,
        _ => return None,
    })
}

/// BIGNUMERIC, RANGE and the MAX-parameterised types, which the parser keeps by name.
fn custom_type(compact: &str) -> Option<String> {
    if let Some(element) = compact
        .strip_prefix("RANGE<")
        .and_then(|rest| rest.strip_suffix('>'))
    {
        return RANGE_ELEMENT_TYPES
            .contains(&element)
            .then(|| compact.to_owned());
    }
    if ["BIGNUMERIC", "BIGNUMERIC(MAX)", "STRING(MAX)", "BYTES(MAX)"].contains(&compact) {
        return Some(compact.to_owned());
    }
    if let Some(scale) = compact
        .strip_prefix("BIGNUMERIC(MAX,")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return parameter(scale)
            .filter(|scale| *scale <= BIGNUMERIC_DIGITS.0)
            .map(|_| compact.to_owned());
    }
    let inner: &str = compact
        .strip_prefix("BIGNUMERIC(")
        .and_then(|rest| rest.strip_suffix(')'))?;
    let given: Vec<u32> = inner.split(',').map(parameter).collect::<Option<_>>()?;
    decimal("BIGNUMERIC", &given, BIGNUMERIC_DIGITS)
}

/// A decimal type within BigQuery's scale and integer-digit bounds.
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

/// A type parameter written in digits.
fn parameter(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    text.chars().try_fold(0_u32, |value, character| {
        let digit: u32 = character.to_digit(DECIMAL_RADIX)?;
        value.checked_mul(DECIMAL_RADIX)?.checked_add(digit)
    })
}

/// A STRING or BYTES type, whose length when given is at least 1.
fn sized(base: &str, length: Option<u32>) -> Option<String> {
    match length {
        None => Some(base.to_owned()),
        Some(0) => None,
        Some(length) => Some(format!("{base}({length})")),
    }
}

/// The type Polyglot gives `spelling`: its name, the text fallback's parameters, and its family.
fn polyglot_type(spelling: &str, data_type: &DataType) -> NormalizedType {
    let name: String = polyglot_name(spelling, data_type, true);
    let read: NormalizedType = normalize_text_type(&name, Some(TypeDialect::BigQuery));
    let family: TypeFamily = match data_type {
        DataType::Array { .. } | DataType::Struct { .. }
            if !polyglot_reads_as_text(spelling, data_type) =>
        {
            TypeFamily::Other
        }
        DataType::Timestamp { timezone: true, .. } => TypeFamily::Timestamp,
        _ => read.family,
    };
    NormalizedType {
        normalized_name: name,
        family,
        ..read
    }
}

/// Whether Polyglot cannot parse nested `spelling`: a RANGE closed by `>>`, or an unnamed RANGE field.
fn polyglot_reads_as_text(spelling: &str, data_type: &DataType) -> bool {
    let upper: String = spelling.to_ascii_uppercase();
    let closed_by_shift: bool = upper.match_indices(RANGE_TYPE_NAME).any(|(at, _)| {
        let rest: &str = upper[at + RANGE_TYPE_NAME.len()..].trim_start();
        rest.starts_with('<')
            && rest
                .find('>')
                .is_some_and(|close| rest[close + 1..].starts_with('>'))
    });
    closed_by_shift || unnamed_range_field(data_type)
}

/// Whether a STRUCT within `data_type` has an unnamed RANGE field.
fn unnamed_range_field(data_type: &DataType) -> bool {
    match data_type {
        DataType::Array { element_type, .. } => unnamed_range_field(element_type),
        DataType::Struct { fields, .. } => fields.iter().any(|field| {
            let range: bool = matches!(
                &field.data_type,
                DataType::Custom { name } if compact(name).starts_with(RANGE_TYPE_NAME)
            );
            field.name.is_empty() && range || unnamed_range_field(&field.data_type)
        }),
        _ => false,
    }
}

/// The name Polyglot gives `spelling`, a type BigQuery accepts parsed as `data_type`.
fn polyglot_name(spelling: &str, data_type: &DataType, top: bool) -> String {
    let text: &str = spelling.trim();
    let inner = || {
        let open: usize = text.find('<').map_or(text.len(), |at| at + 1);
        let close: usize = text.rfind('>').unwrap_or(text.len()).max(open);
        &text[open..close]
    };
    match data_type {
        DataType::Array { element_type, .. } => {
            format!("ARRAY<{}>", polyglot_name(inner(), element_type, false))
        }
        DataType::Struct { fields, .. } => {
            let named: Vec<String> = top_level_fields(inner())
                .into_iter()
                .zip(fields)
                .map(|(field_text, field)| {
                    if field.name.is_empty() && field_text.contains('(') {
                        return compact(field_text);
                    }
                    if field.name.is_empty() {
                        return polyglot_name(field_text, &field.data_type, false);
                    }
                    let (name, rest) = split_field_name(field_text.trim_start());
                    format!(
                        "{}{}",
                        compact(name),
                        polyglot_name(rest, &field.data_type, false)
                    )
                })
                .collect();
            format!("STRUCT<{}>", named.join(","))
        }
        _ => {
            let spelt: String = compact(text);
            let base_end: usize = spelt.find(['(', '<']).unwrap_or(spelt.len());
            let (base, parameters) = spelt.split_at(base_end);
            let names: &[(&str, &str)] = if top {
                POLYGLOT_TOP_LEVEL_TYPE_NAMES
            } else {
                POLYGLOT_NESTED_TYPE_NAMES
            };
            let parameters: String = match parameters.strip_prefix("(MAX") {
                Some(rest) if top && POLYGLOT_MAX_DROPPED.contains(&base) => rest
                    .strip_prefix(',')
                    .map_or(String::new(), |scale| format!("({scale}")),
                _ => parameters.to_owned(),
            };
            match names.iter().find(|(alias, _)| *alias == base) {
                Some((_, name)) => format!("{name}{parameters}"),
                None => format!("{base}{parameters}"),
            }
        }
    }
}

/// Upper case without whitespace, as Polyglot's generated type text compares.
fn compact(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// A STRUCT's field texts: its commas outside brackets, parentheses and quoted names.
fn top_level_fields(text: &str) -> Vec<&str> {
    let mut fields: Vec<&str> = Vec::new();
    let (mut depth, mut quote, mut start): (usize, Option<char>, usize) = (0, None, 0);
    for (at, character) in text.char_indices() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '`' | '\'' | '"') => quote = Some(character),
            (None, '<' | '(') => depth += 1,
            (None, '>' | ')') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                fields.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    fields.push(&text[start..]);
    fields
}

/// A named field's name, backquoted or a word, and its type text.
fn split_field_name(text: &str) -> (&str, &str) {
    let end: usize = match text.strip_prefix('`') {
        Some(rest) => rest.find('`').map_or(text.len(), |at| at + 2),
        None => text
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(text.len()),
    };
    text.split_at(end)
}
