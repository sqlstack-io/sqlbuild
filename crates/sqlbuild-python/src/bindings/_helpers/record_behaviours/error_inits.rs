//! Constructors and derived copies of the natively hosted expected-error classes.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use sqlbuild_python_base::boundary::main::call_argument::call_argument;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const HELP_FIELD: &str = "help";
const LOCATION_FIELD: &str = "location";
/// `MacroArgumentError` keyword fields, in constructor order.
const MACRO_ARGUMENT_FIELDS: [&str; 7] = [
    "detail",
    "help",
    "macro_name",
    "file_path",
    "offset",
    "relative_position",
    "location",
];

/// Set the expected-error attributes every native error carries.
fn initialized(
    error: &Bound<'_, PyAny>,
    message: &Bound<'_, PyAny>,
    help: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = error.py();
    error.setattr("args", PyTuple::new(py, [message])?)?;
    error.setattr("message", message)?;
    if let Ok(code) = native_field(&error.get_type(), "code") {
        error.setattr("code", code)?;
    }
    error.setattr("help", help)
}

/// A keyword argument, or `None` when absent.
fn keyword<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let value: Option<Bound<'py, PyAny>> = match kwargs {
        Some(kwargs) => kwargs.get_item(name)?,
        None => None,
    };
    Ok(value.unwrap_or_else(|| py.None().into_bound(py)))
}

/// `MacroArgumentError(detail=..., help=..., macro_name=..., file_path=..., offset=..., relative_position=..., location=None)`.
pub(crate) fn macro_argument_error_init(
    error: &Bound<'_, PyAny>,
    _args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = error.py();
    let field = |name: &str| keyword(py, kwargs, name);
    let location: Bound<'_, PyAny> = field("location")?;
    let file_path: String = field("file_path")?.str()?.extract()?;
    let macro_name: String = field("macro_name")?.str()?.extract()?;
    let detail: String = field("detail")?.str()?.extract()?;
    let (line, column): (Bound<'_, PyAny>, Bound<'_, PyAny>) = if location.is_none() {
        let position: Bound<'_, PyAny> = field("relative_position")?;
        (position.get_item(0)?, position.get_item(1)?)
    } else {
        (
            native_field(&location, "line")?,
            native_field(&location, "column")?,
        )
    };
    let call: String = if macro_name.starts_with('@') {
        macro_name
    } else {
        format!("@py.{macro_name}")
    };
    let (scope, suffix): (String, String) = if location.is_none() {
        (
            format!("in '{file_path}'"),
            format!(" of the '{call}' arguments"),
        )
    } else {
        (format!("of '{call}' in '{file_path}'"), String::new())
    };
    let message: String = format!(
        "Macro arguments {scope} {detail} at line {}, column {}{suffix}",
        line.str()?,
        column.str()?
    );
    initialized(error, PyString::new(py, &message).as_any(), &field("help")?)?;
    for name in MACRO_ARGUMENT_FIELDS {
        if name != HELP_FIELD {
            error.setattr(name, field(name)?)?;
        }
    }
    Ok(py.None())
}

/// A copy of `error` with `changes` applied to its constructor fields.
fn macro_argument_copy<'py>(
    error: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = error.py();
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    for name in MACRO_ARGUMENT_FIELDS {
        if name == LOCATION_FIELD {
            continue;
        }
        keywords.set_item(name, native_field(error, name)?)?;
    }
    let help: Bound<'py, PyAny> = native_field(error, "help")?;
    if !help.is_truthy()? {
        keywords.set_item("help", "")?;
    }
    for (name, value) in changes {
        keywords.set_item(*name, value)?;
    }
    error.get_type().call((), Some(&keywords))
}

/// `MacroArgumentError.shifted(by)`: the same error, its offset moved into the enclosing SQL.
pub(crate) fn macro_argument_error_shifted(
    error: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let by: Bound<'_, PyAny> = call_argument((args, kwargs), "by", 0)?;
    let offset: Bound<'_, PyAny> = native_field(error, "offset")?.add(by)?;
    Ok(macro_argument_copy(error, &[("offset", offset)])?.unbind())
}

/// `MacroArgumentError.located(location)`: the same error at its authored file location.
pub(crate) fn macro_argument_error_located(
    error: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let location: Bound<'_, PyAny> = call_argument((args, kwargs), "location", 0)?;
    Ok(macro_argument_copy(error, &[("location", location)])?.unbind())
}

/// `SqlTestReferenceError(message, *, location, help)`: the message prefixed with its location.
pub(crate) fn sql_test_reference_error_init(
    error: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = error.py();
    let message: Bound<'_, PyAny> = call_argument((args, kwargs), "message", 0)?;
    let location: Bound<'_, PyAny> = keyword(py, kwargs, "location")?;
    let path: Bound<'_, PyAny> = native_field(&location, "path")?.call_method0("as_posix")?;
    let text: String = format!(
        "{}:{}:{}: {}",
        path.str()?,
        native_field(&location, "line")?.str()?,
        native_field(&location, "column")?.str()?,
        message.str()?
    );
    initialized(
        error,
        PyString::new(py, &text).as_any(),
        &keyword(py, kwargs, "help")?,
    )?;
    error.setattr("location", location)?;
    Ok(py.None())
}

/// `RulesConfigError(message, *, help)`.
pub(crate) fn rules_config_error_init(
    error: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = error.py();
    let message: Bound<'_, PyAny> = call_argument((args, kwargs), "message", 0)?;
    error.setattr("args", PyTuple::new(py, [&message])?)?;
    error.setattr("message", message)?;
    error.setattr("help", keyword(py, kwargs, "help")?)?;
    Ok(py.None())
}
