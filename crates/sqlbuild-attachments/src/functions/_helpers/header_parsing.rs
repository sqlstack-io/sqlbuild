//! Python's header checks in the order its attachment raises them, with their messages.

use crate::functions::_helpers::values::{
    TABLE_RETURN_KEY, description, lookup, named_types, stripped_list, stripped_text,
};
use crate::functions::errors::NamedTypeError;
use crate::functions::models::{
    FunctionHeader, FunctionLanguage, FunctionReturns, HeaderFailure, HeaderStage, HeaderValue,
    NamedType,
};

/// Python's error messages for one function file.
pub(crate) struct HeaderErrors<'path> {
    pub(crate) language: FunctionLanguage,
    /// A Python `@table_fn(...)` file, whose `returns` maps each column to its type.
    pub(crate) table_function: bool,
    pub(crate) relative_path: &'path str,
}

impl HeaderErrors<'_> {
    fn fail(&self, stage: HeaderStage, problem: &str) -> HeaderFailure {
        let language: &str = match self.language {
            FunctionLanguage::Sql => "SQL",
            FunctionLanguage::Python => "Python",
        };
        HeaderFailure {
            stage,
            message: format!("{language} function file {} {problem}", self.relative_path),
        }
    }

    fn named_type(&self, stage: HeaderStage, error: NamedTypeError, kind: &str) -> HeaderFailure {
        match error {
            NamedTypeError::InvalidName => self.fail(stage, &format!("has an invalid {kind} name")),
            NamedTypeError::MissingType(name) => {
                self.fail(stage, &format!("{kind} '{name}' must declare a type"))
            }
        }
    }

    fn list(&self, stage: HeaderStage, key: &str, not_list: bool) -> HeaderFailure {
        if not_list {
            self.fail(stage, &format!("{key} must be a list"))
        } else {
            self.fail(stage, &format!("{key} entries must be non-empty strings"))
        }
    }
}

/// The header as Python parses it, stopping at its first error.
pub(crate) fn parsed_header(
    header: &[(String, HeaderValue)],
    errors: &HeaderErrors<'_>,
) -> FunctionHeader {
    let python: bool = errors.language == FunctionLanguage::Python;
    let python_table: bool = python && errors.table_function;
    let mut parsed = FunctionHeader {
        arguments: Vec::new(),
        returns: None,
        tags: Vec::new(),
        description: None,
        runtime_version: None,
        entry_point: None,
        packages: Vec::new(),
        failure: None,
    };
    let raw_returns: Option<&HeaderValue> = lookup(header, "returns");
    let scalar_returns: Option<String> = stripped_text(raw_returns);
    if raw_returns.is_none() || (python && !python_table && scalar_returns.is_none()) {
        return failed(
            parsed,
            errors.fail(HeaderStage::Start, "must declare returns"),
        );
    }
    match lookup(header, "arguments") {
        None => {}
        Some(HeaderValue::Map(entries)) => {
            let (arguments, error) = named_types(entries);
            parsed.arguments = arguments;
            if let Some(error) = error {
                let failure = errors.named_type(HeaderStage::Arguments, error, "argument");
                return failed(parsed, failure);
            }
        }
        Some(_) => {
            let failure = errors.fail(HeaderStage::Arguments, "arguments must be a map");
            return failed(parsed, failure);
        }
    }
    if python_table {
        let (columns, failure) = python_table_returns(raw_returns, errors);
        parsed.returns = Some(FunctionReturns::Table(columns));
        if let Some(failure) = failure {
            return failed(parsed, failure);
        }
    } else if let Some(text) = scalar_returns {
        parsed.returns = Some(FunctionReturns::Type(text));
    } else {
        let (columns, failure) = table_returns(raw_returns, errors);
        parsed.returns = Some(FunctionReturns::Table(columns));
        if let Some(failure) = failure {
            return failed(parsed, failure);
        }
    }
    if python {
        parsed.runtime_version = stripped_text(lookup(header, "runtime_version"));
        if parsed.runtime_version.is_none() {
            let failure = errors.fail(HeaderStage::PythonValues, "must declare runtime_version");
            return failed(parsed, failure);
        }
        parsed.entry_point = stripped_text(lookup(header, "entry_point"));
        if parsed.entry_point.is_none() {
            let failure = errors.fail(HeaderStage::PythonValues, "must declare entry_point");
            return failed(parsed, failure);
        }
        match stripped_list(lookup(header, "packages")) {
            Ok(packages) => parsed.packages = packages,
            Err(not_list) => {
                let failure = errors.list(HeaderStage::PythonValues, "packages", not_list);
                return failed(parsed, failure);
            }
        }
    }
    match stripped_list(lookup(header, "tags")) {
        Ok(tags) => parsed.tags = tags,
        Err(not_list) => {
            let failure = errors.list(HeaderStage::Metadata, "tags", not_list);
            return failed(parsed, failure);
        }
    }
    match description(lookup(header, "description")) {
        Some(text) => parsed.description = text,
        None => {
            let failure = errors.fail(HeaderStage::Metadata, "description must be a string");
            return failed(parsed, failure);
        }
    }
    parsed
}

fn failed(parsed: FunctionHeader, failure: HeaderFailure) -> FunctionHeader {
    FunctionHeader {
        failure: Some(failure),
        ..parsed
    }
}

/// A `@table_fn(returns={...})` column map, which must name at least one column.
fn python_table_returns(
    raw_returns: Option<&HeaderValue>,
    errors: &HeaderErrors<'_>,
) -> (Vec<NamedType>, Option<HeaderFailure>) {
    let shape_error = || {
        errors.fail(
            HeaderStage::Returns,
            "returns must map each column name to its type",
        )
    };
    match raw_returns {
        Some(HeaderValue::Map(entries)) if !entries.is_empty() => {
            let (columns, error) = named_types(entries);
            let failure =
                error.map(|error| errors.named_type(HeaderStage::Returns, error, "return column"));
            (columns, failure)
        }
        _ => (Vec::new(), Some(shape_error())),
    }
}

/// Python's `returns (table (...))` columns: exactly the `table` key with a non-empty column map.
fn table_returns(
    raw_returns: Option<&HeaderValue>,
    errors: &HeaderErrors<'_>,
) -> (Vec<NamedType>, Option<HeaderFailure>) {
    let shape_error = || {
        errors.fail(
            HeaderStage::Returns,
            "returns must be a type string or table column declaration",
        )
    };
    let Some(HeaderValue::Map(entries)) = raw_returns else {
        return (Vec::new(), Some(shape_error()));
    };
    let [(HeaderValue::Text(key), table)] = entries.as_slice() else {
        return (Vec::new(), Some(shape_error()));
    };
    if key != TABLE_RETURN_KEY {
        return (Vec::new(), Some(shape_error()));
    }
    match table {
        HeaderValue::Map(entries) if !entries.is_empty() => {
            let (columns, error) = named_types(entries);
            let failure =
                error.map(|error| errors.named_type(HeaderStage::Returns, error, "return column"));
            (columns, failure)
        }
        _ => (
            Vec::new(),
            Some(errors.fail(
                HeaderStage::Returns,
                "returns table must declare at least one column",
            )),
        ),
    }
}
