//! The parse's interpreter side without an interpreter: Python's `int`, `repr` and the version.

use sqlbuild_core::text::main::decimal_digit_value::decimal_digit_value;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::argv::main::cli_namespace_defaults::cli_namespace_defaults;
use crate::argv::models::{NamespaceDefault, TypeFailure, Value};
use crate::argv::types::ArgvHost;
use crate::launcher::constants::PROGRAM_VERSION;
use crate::native_commands::constants::{
    DIGIT_SEPARATOR, INT_ASCII_SPACES, INT_CONVERTER, NEGATIVE_SIGN, POSITIVE_INTEGER_CONVERTER,
    POSITIVE_INTEGER_MESSAGE, POSITIVE_SIGN, PYTHON_INT_MAX_STR_DIGITS, ZERO_TEXT,
};

/// The ASCII text CPython parses: other decimal digits become ASCII, other spaces a space.
fn ascii_digits_and_spaces(text: &str) -> Option<String> {
    text.chars()
        .map(|character| {
            if character.is_ascii() {
                Some(character)
            } else if python_isspace(character) {
                Some(' ')
            } else {
                decimal_digit_value(character).and_then(|digit| char::from_digit(digit, 10))
            }
        })
        .collect()
}

/// The canonical decimal text of Python's `int(text)`, or `None` where it raises `ValueError`.
pub(crate) fn python_int(text: &str) -> Option<String> {
    let ascii: String = ascii_digits_and_spaces(text)?;
    let trimmed: &str = ascii.trim_matches(|character: char| INT_ASCII_SPACES.contains(character));
    let (negative, body): (bool, &str) = match trimmed.strip_prefix(NEGATIVE_SIGN) {
        Some(body) => (true, body),
        None => (
            false,
            trimmed.strip_prefix(POSITIVE_SIGN).unwrap_or(trimmed),
        ),
    };
    let mut digits: String = String::with_capacity(body.len());
    let mut after_digit: bool = false;
    for character in body.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            after_digit = true;
        } else if character == DIGIT_SEPARATOR && after_digit {
            after_digit = false;
        } else {
            return None;
        }
    }
    if !after_digit || digits.len() > PYTHON_INT_MAX_STR_DIGITS {
        return None;
    }
    let significant: &str = digits.trim_start_matches('0');
    Some(match (significant.is_empty(), negative) {
        (true, _) => ZERO_TEXT.to_owned(),
        (false, true) => format!("{NEGATIVE_SIGN}{significant}"),
        (false, false) => significant.to_owned(),
    })
}

/// Converts what it can as Python would and records any converter only Python can apply.
#[derive(Debug, Default)]
pub(crate) struct NativeArgvHost {
    /// Whether the parse met a converter, such as `--vars`, that needs the interpreter.
    pub(crate) needs_python: bool,
}

impl ArgvHost for NativeArgvHost {
    fn convert(&mut self, converter: &str, text: &str) -> Result<Value, TypeFailure> {
        match converter {
            INT_CONVERTER => python_int(text).map(Value::Int).ok_or(TypeFailure::Invalid),
            POSITIVE_INTEGER_CONVERTER => {
                let number: String = python_int(text).ok_or(TypeFailure::Invalid)?;
                if number == ZERO_TEXT || number.starts_with(NEGATIVE_SIGN) {
                    return Err(TypeFailure::ArgumentType(
                        POSITIVE_INTEGER_MESSAGE.to_owned(),
                    ));
                }
                Ok(Value::Int(number))
            }
            _ => {
                self.needs_python = true;
                Ok(Value::None)
            }
        }
    }

    fn repr(&self, text: &str) -> String {
        python_str_repr(text)
    }

    fn program_version(&mut self) -> String {
        PROGRAM_VERSION.to_owned()
    }
}

/// The parse's starting namespace: every `CliNamespace` default.
pub(crate) fn namespace_defaults() -> Vec<(String, Value)> {
    cli_namespace_defaults()
        .iter()
        .map(|(name, default)| {
            let value: Value = match default {
                NamespaceDefault::None => Value::None,
                NamespaceDefault::Bool(flag) => Value::Bool(*flag),
                NamespaceDefault::Int(number) => Value::Int(number.to_string()),
                NamespaceDefault::Text(text) => Value::Str((*text).to_owned()),
                NamespaceDefault::EmptyList => Value::List(Vec::new()),
                NamespaceDefault::EmptyDict => Value::EmptyDict,
                NamespaceDefault::CompileLineageFast | NamespaceDefault::ColumnLineageRich => {
                    Value::Host(0)
                }
            };
            ((*name).to_owned(), value)
        })
        .collect()
}
