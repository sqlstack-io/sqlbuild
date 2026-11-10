//! DSL header values parsed natively for header rules, compared as Python compares their objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;
use sqlbuild_rules::dsl_headers::main::inner_header_text::inner_header_text;
use sqlbuild_rules::dsl_headers::models::{HeaderSpan, ParseFailure, ParseOutcome};
use sqlbuild_rules::dsl_headers::types::HeaderParser;
use sqlbuild_sqltext::compiler::main::header_word_value::header_word_value;
use sqlbuild_sqltext::compiler::main::model_header_single_parsing::parse_one;
use sqlbuild_sqltext::compiler::models::{AuthoredValue, HeaderWord};
use sqlbuild_sqltext::header_strings::main::quote_failure::quote_failure;
use sqlbuild_sqltext::header_strings::models::QuoteFailure;

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::sqltext::model_headers::header_failure;

/// The path header rules report header syntax errors at.
const SENTINEL_PATH: &str = "<lint>";
const DESCRIPTION_KEY: &str = "description";
pub(crate) const FUNCTION_KIND: &str = "FUNCTION";
const ARGUMENTS_KEY: &str = "arguments";
const NEGATIVE_ZERO: &str = "-0";
const ZERO: &str = "0";

/// Header values parsed natively, compared as their Python projections compare.
#[derive(Default)]
pub(crate) struct NativeHeaderParser;

/// The `ModelSqlParseError` a rejected header parse raises.
pub(crate) fn rejection(py: Python<'_>, failure: ParseFailure) -> PyErr {
    let raised: PyResult<PyErr> = (|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", failure.help)?;
        let error: Bound<'_, PyAny> =
            error_class(py, ErrorId::ModelSqlParse)?.call((failure.message,), Some(&keywords))?;
        Ok(PyErr::from_value(error))
    })();
    raised.unwrap_or_else(|error| error)
}

impl HeaderParser for NativeHeaderParser {
    type Values = AuthoredValue;
    /// A native failure's message.
    type Error = String;

    fn parse(&self, kind: &str, inner: &str) -> ParseOutcome<AuthoredValue, String> {
        let parsed = match catch_compiler_panic(|| Ok(parse_one(inner))) {
            Ok(parsed) => parsed,
            Err(message) => {
                return ParseOutcome::Raised(
                    message.clone(),
                    ParseFailure {
                        message,
                        help: None,
                        position: None,
                    },
                );
            }
        };
        let (value, _, error) = parsed;
        if let Some(value) = value {
            return ParseOutcome::Parsed(value);
        }
        let error: String = error.unwrap_or_else(|| "None".to_owned());
        let quote: Option<QuoteFailure> = quote_failure(&error, inner, (1, 1));
        let (detail, help, _) = header_failure(&error, inner, None);
        let failure: ParseFailure = ParseFailure {
            message: format!(
                "{kind}(...) in '{SENTINEL_PATH}' contains invalid SQLBuild header syntax: {detail}"
            ),
            help,
            position: quote.map(|quote| quote.position),
        };
        ParseOutcome::Rejected(failure)
    }

    fn description(&self, values: &AuthoredValue) -> Result<Option<String>, String> {
        Ok(match map_value(values, DESCRIPTION_KEY) {
            Some(AuthoredValue::String(text)) => Some(text.clone()),
            Some(AuthoredValue::BareWord(word)) if header_word_value(word) == HeaderWord::Text => {
                Some(word.clone())
            }
            _ => None,
        })
    }

    fn same(&self, left: &AuthoredValue, right: &AuthoredValue) -> Result<bool, String> {
        Ok(python_equal(left, right))
    }
}

/// Why a header could not be read: a header syntax error, or a native failure's message.
pub(crate) enum HeaderFailure {
    Rejected(ParseFailure),
    Native(String),
}

impl HeaderFailure {
    pub(crate) fn into_pyerr(self, py: Python<'_>) -> PyErr {
        match self {
            Self::Rejected(failure) => rejection(py, failure),
            Self::Native(message) => value_error(message),
        }
    }
}

/// The SQL function argument names the first FUNCTION header declares, lowercased and sorted.
pub(crate) fn external_identifiers(
    text: &[char],
    headers: &[HeaderSpan],
    parser: &NativeHeaderParser,
) -> Result<Vec<String>, HeaderFailure> {
    for header in headers {
        if header.kind != FUNCTION_KIND {
            continue;
        }
        let inner: String = inner_header_text(&text[header.start..header.end])
            .iter()
            .collect();
        let values: AuthoredValue = match parser.parse(&header.kind, &inner) {
            ParseOutcome::Parsed(values) => values,
            ParseOutcome::Rejected(failure) => return Err(HeaderFailure::Rejected(failure)),
            ParseOutcome::Raised(message, _) => return Err(HeaderFailure::Native(message)),
        };
        let Some(arguments) = map_value(&values, ARGUMENTS_KEY) else {
            continue;
        };
        if !matches!(arguments, AuthoredValue::Map(_)) {
            continue;
        }
        let mut names: Vec<String> = map_keys(arguments)
            .into_iter()
            .map(str::to_lowercase)
            .collect();
        names.sort_unstable();
        return Ok(names);
    }
    Ok(Vec::new())
}

/// The value a header map projects for `key`: its last entry, as a Python dict keeps it.
pub(crate) fn map_value<'a>(values: &'a AuthoredValue, key: &str) -> Option<&'a AuthoredValue> {
    match values {
        AuthoredValue::Map(entries) => entries
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value),
        _ => None,
    }
}

/// A header map's keys once each, as the projected Python dict holds them.
pub(crate) fn map_keys(values: &AuthoredValue) -> Vec<&str> {
    let mut keys: Vec<&str> = Vec::new();
    if let AuthoredValue::Map(entries) = values {
        for (name, _) in entries {
            if !keys.contains(&name.as_str()) {
                keys.push(name);
            }
        }
    }
    keys
}

/// A scalar as its projected Python object compares.
#[derive(Debug, PartialEq)]
enum Scalar<'a> {
    None,
    Integer(i128),
    BigInteger(String),
    Float(f64),
    Text(&'a str),
}

fn scalar(value: &AuthoredValue) -> Option<Scalar<'_>> {
    Some(match value {
        AuthoredValue::Null => Scalar::None,
        AuthoredValue::Boolean(flag) => Scalar::Integer(i128::from(*flag)),
        AuthoredValue::String(text) => Scalar::Text(text),
        AuthoredValue::BareWord(word) => match header_word_value(word) {
            HeaderWord::Bool(flag) => Scalar::Integer(i128::from(flag)),
            HeaderWord::Null => Scalar::None,
            HeaderWord::Integer(digits) => match digits.parse::<i128>() {
                Ok(number) => Scalar::Integer(number),
                Err(_) => Scalar::BigInteger(digits),
            },
            HeaderWord::Float(number) => Scalar::Float(number),
            HeaderWord::Text => Scalar::Text(word),
        },
        _ => return None,
    })
}

fn scalars_equal(left: &Scalar<'_>, right: &Scalar<'_>) -> bool {
    match (left, right) {
        (Scalar::Integer(integer), Scalar::Float(float))
        | (Scalar::Float(float), Scalar::Integer(integer)) => {
            integral_float_text(*float).is_some_and(|text| text == integer.to_string())
        }
        (Scalar::BigInteger(digits), Scalar::Float(float))
        | (Scalar::Float(float), Scalar::BigInteger(digits)) => {
            integral_float_text(*float).is_some_and(|text| text == digits.trim_start_matches('+'))
        }
        (Scalar::Float(left), Scalar::Float(right)) => left == right,
        (left, right) => left == right,
    }
}

/// The exact decimal digits of an integral finite float, as Python compares it with an int.
fn integral_float_text(float: f64) -> Option<String> {
    if !float.is_finite() || float.trunc() != float {
        return None;
    }
    let text: String = format!("{float:.0}");
    Some(if text == NEGATIVE_ZERO {
        ZERO.to_owned()
    } else {
        text
    })
}

/// Whether two parsed header values project to equal Python objects.
pub(crate) fn python_equal(left: &AuthoredValue, right: &AuthoredValue) -> bool {
    if let (Some(left), Some(right)) = (scalar(left), scalar(right)) {
        return scalars_equal(&left, &right);
    }
    match (left, right) {
        (AuthoredValue::List(left), AuthoredValue::List(right))
        | (AuthoredValue::Tuple(left), AuthoredValue::Tuple(right))
        | (AuthoredValue::Set(left), AuthoredValue::Set(right)) => sequences_equal(left, right),
        (AuthoredValue::Map(_), AuthoredValue::Map(_)) => maps_equal(left, right),
        (AuthoredValue::TypedConstant(left), AuthoredValue::TypedConstant(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| left.0 == right.0 && python_equal(&left.1, &right.1))
        }
        (AuthoredValue::InlineSqlHook(left), AuthoredValue::InlineSqlHook(right)) => left == right,
        (
            AuthoredValue::NamedSqlHook(left_name, left_kwargs),
            AuthoredValue::NamedSqlHook(right_name, right_kwargs),
        )
        | (
            AuthoredValue::PythonHook(left_name, left_kwargs),
            AuthoredValue::PythonHook(right_name, right_kwargs),
        ) => {
            left_name == right_name
                && maps_equal(
                    &AuthoredValue::Map(left_kwargs.clone()),
                    &AuthoredValue::Map(right_kwargs.clone()),
                )
        }
        _ => false,
    }
}

fn sequences_equal(left: &[AuthoredValue], right: &[AuthoredValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| python_equal(left, right))
}

fn maps_equal(left: &AuthoredValue, right: &AuthoredValue) -> bool {
    let (left_keys, right_keys) = (map_keys(left), map_keys(right));
    left_keys.len() == right_keys.len()
        && left_keys
            .iter()
            .all(|key| match (map_value(left, key), map_value(right, key)) {
                (Some(left), Some(right)) => python_equal(left, right),
                _ => false,
            })
}
