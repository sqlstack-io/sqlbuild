//! `sqb`'s argument parsing: the native parser with Python converters and output.

use pyo3::exceptions::{PyAttributeError, PyOSError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyInt, PyList, PyListMethods, PyString,
};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::review_command::constants::{JSON_OPTION, REVIEW_PROG, USAGE_ERROR_CODE};
use sqlbuild_analysis::review_command::main::render_review_json::render_review_json;
use sqlbuild_analysis::review_command::models::{ReviewFailure, ReviewReport};
use sqlbuild_cli::argv::main::parse_cli_arguments::parse_cli_arguments as parse_arguments;
use sqlbuild_cli::argv::models::{
    ArgvOutcome, ArgvSettings, Stream, TypeFailure, UsageFailure, Value,
};
use sqlbuild_cli::argv::types::ArgvHost;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::bindings::_helpers::cli::namespace::new_cli_namespace;
use crate::bindings::_helpers::cli::surrogates::{python_text, rust_text, uses_placeholders};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_output::recall::main::write_stdout_bytes::write_stdout_bytes;

const INT_CONVERTER: &str = "int";
const POSITIVE_INTEGER_CONVERTER: &str = "_positive_integer";
const VARS_CONVERTER: &str = "parse_cli_vars";
const DISTRIBUTION: &str = "sqlbuild";
const UNKNOWN_VERSION: &str = "unknown";

/// Parsed attributes as `(name, value)` pairs, or `None`; then the exit status, or `None`.
type ParsedRows = (Option<Vec<(String, Py<PyAny>)>>, Option<i64>);

/// The interpreter side of a parse: converters, `repr` and the installed version.
struct PythonHost<'py> {
    py: Python<'py>,
    objects: Vec<Py<PyAny>>,
    /// Whether command-line text carries lone surrogates as placeholders.
    surrogates: bool,
}

impl<'py> PythonHost<'py> {
    /// Rust text as the Python `str` it came from.
    fn text(&self, text: &str) -> PyResult<Bound<'py, PyAny>> {
        python_text(self.py, text, self.surrogates)
    }

    fn keep(&mut self, object: Bound<'_, PyAny>) -> Value {
        self.objects.push(object.unbind());
        Value::Host(self.objects.len() - 1)
    }

    fn integer(&self, text: &str) -> Result<Bound<'_, PyAny>, TypeFailure> {
        self.text(text)
            .and_then(|text| self.py.get_type::<PyInt>().call1((text,)))
            .map_err(|_| TypeFailure::Invalid)
    }

    fn vars(&mut self, text: &str) -> PyResult<Result<Value, TypeFailure>> {
        let json: Bound<'_, PyModule> = self.py.import("json")?;
        let parsed: Bound<'_, PyAny> =
            match native_field(&json, "loads")?.call1((self.text(text)?,)) {
                Ok(parsed) => parsed,
                Err(error) => {
                    let decode_error: Bound<'_, PyAny> = native_field(&json, "JSONDecodeError")?;
                    if !error.value(self.py).is_instance(&decode_error)? {
                        return Err(error);
                    }
                    let message: String = native_field(error.value(self.py), "msg")?.extract()?;
                    return Ok(Err(TypeFailure::ArgumentType(format!(
                        "--vars must be valid JSON: {message}"
                    ))));
                }
            };
        let Ok(parsed) = parsed.downcast_into::<PyDict>() else {
            return Ok(Err(TypeFailure::ArgumentType(
                "--vars must be a JSON object".to_owned(),
            )));
        };
        Ok(Ok(self.keep(parsed.into_any())))
    }
}

impl ArgvHost for PythonHost<'_> {
    fn convert(&mut self, converter: &str, text: &str) -> Result<Value, TypeFailure> {
        match converter {
            INT_CONVERTER => {
                let number: Bound<'_, PyAny> = self.integer(text)?;
                Ok(Value::Int(
                    number.str().map_err(|_| TypeFailure::Invalid)?.to_string(),
                ))
            }
            POSITIVE_INTEGER_CONVERTER => {
                let number: Bound<'_, PyAny> = self.integer(text)?;
                if number.lt(1).unwrap_or(true) {
                    return Err(TypeFailure::ArgumentType(
                        "value must be a positive integer".to_owned(),
                    ));
                }
                Ok(Value::Int(
                    number.str().map_err(|_| TypeFailure::Invalid)?.to_string(),
                ))
            }
            VARS_CONVERTER => self.vars(text).unwrap_or(Err(TypeFailure::Invalid)),
            _ => Err(TypeFailure::Invalid),
        }
    }

    fn repr(&self, text: &str) -> String {
        self.text(text)
            .and_then(|text| text.repr())
            .map(|repr| repr.to_string())
            .unwrap_or_else(|_| python_str_repr(text))
    }

    fn program_version(&mut self) -> String {
        self.py
            .import("importlib.metadata")
            .and_then(|metadata| native_field(&metadata, "version")?.call1((DISTRIBUTION,)))
            .and_then(|version| version.extract())
            .unwrap_or_else(|_| UNKNOWN_VERSION.to_owned())
    }
}

impl PythonHost<'_> {
    /// A Python default as a parse value; anything the parse does not read stays a host object.
    fn value_of(&mut self, object: &Bound<'_, PyAny>) -> PyResult<Value> {
        if object.is_none() {
            return Ok(Value::None);
        }
        if let Ok(flag) = object.downcast::<PyBool>() {
            return Ok(Value::Bool(flag.is_true()));
        }
        if object.is_exact_instance_of::<PyInt>() {
            return Ok(Value::Int(object.str()?.to_string()));
        }
        if object.is_exact_instance_of::<PyString>() {
            return Ok(Value::Str(object.extract()?));
        }
        if let Ok(items) = object.downcast_exact::<PyList>() {
            let mut values: Vec<Value> = Vec::with_capacity(items.len());
            for item in items.iter() {
                values.push(self.value_of(&item)?);
            }
            return Ok(Value::List(values));
        }
        if let Ok(dict) = object.downcast_exact::<PyDict>()
            && dict.is_empty()
        {
            return Ok(Value::EmptyDict);
        }
        Ok(self.keep(object.clone()))
    }
}

/// A parse value as the Python object `argparse` would have stored.
fn object_of(py: Python<'_>, host: &PythonHost<'_>, value: Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        Value::None => py.None(),
        Value::Bool(flag) => PyBool::new(py, flag).to_owned().into_any().unbind(),
        Value::Int(text) => py.get_type::<PyInt>().call1((text,))?.unbind(),
        Value::Str(text) => host.text(&text)?.unbind(),
        Value::Path(text) => py
            .import("pathlib")?
            .getattr("Path")?
            .call1((host.text(&text)?,))?
            .unbind(),
        Value::List(items) => {
            let list: Bound<'_, PyList> = PyList::empty(py);
            for item in items {
                list.append(object_of(py, host, item)?)?;
            }
            list.into_any().unbind()
        }
        Value::EmptyDict => PyDict::new(py).into_any().unbind(),
        Value::Host(index) => host
            .objects
            .get(index)
            .map(|object| object.clone_ref(py))
            .ok_or_else(|| PyValueError::new_err("unknown parsed value handle"))?,
    })
}

/// The `sqb review --json` document of a review usage error.
fn review_usage_document(arguments: &[String], outcome: &ArgvOutcome) -> Option<String> {
    let UsageFailure { prog, message } = outcome.usage_failure.as_ref()?;
    if prog != REVIEW_PROG || !arguments.iter().any(|arg| arg == JSON_OPTION) {
        return None;
    }
    Some(render_review_json(&ReviewReport {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        error: Some(ReviewFailure {
            code: USAGE_ERROR_CODE.to_owned(),
            message: message.clone(),
            help: Some("run sqb review --help for the accepted options".to_owned()),
        }),
        ..ReviewReport::default()
    }))
}

/// Parse `argv` onto `defaults`, printing help, version and errors; the attributes or exit status.
fn parse_cli_arguments(
    argv: &[Bound<'_, PyString>],
    defaults: &Bound<'_, PyList>,
    settings: ArgvSettings,
) -> PyResult<ParsedRows> {
    let py: Python<'_> = defaults.py();
    let mut arguments: Vec<String> = Vec::with_capacity(argv.len());
    let mut plain_placeholders: bool = false;
    let mut surrogates: bool = false;
    for arg in argv {
        let (text, mapped) = rust_text(arg)?;
        surrogates |= mapped;
        plain_placeholders |= !mapped && uses_placeholders(&text);
        arguments.push(text);
    }
    if surrogates && plain_placeholders {
        return Err(PyValueError::new_err(
            "a command-line argument mixes undecodable bytes with U+10F800..U+10FFFF",
        ));
    }
    let mut host: PythonHost<'_> = PythonHost {
        py,
        objects: Vec::new(),
        surrogates,
    };
    let mut values: Vec<(String, Value)> = Vec::with_capacity(defaults.len());
    for item in defaults.iter() {
        let (name, object): (String, Bound<'_, PyAny>) = item.extract()?;
        values.push((name, host.value_of(&object)?));
    }
    let outcome: ArgvOutcome = parse_arguments(&arguments, values, settings, &mut host);
    let review_usage: Option<String> = review_usage_document(&arguments, &outcome);
    let sys: Bound<'_, PyModule> = py.import("sys")?;
    for (stream, text) in outcome.writes {
        let name: &str = match stream {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        };
        let file: Bound<'_, PyAny> = native_field(&sys, name)?;
        if file.is_none() {
            continue;
        }
        if let Err(error) = file.call_method1("write", (host.text(&text)?,))
            && !(error.is_instance_of::<PyAttributeError>(py)
                || error.is_instance_of::<PyOSError>(py))
        {
            return Err(error);
        }
    }
    if let Some(document) = review_usage {
        write_stdout_bytes(py, document.as_bytes())?;
    }
    match outcome.parsed {
        Err(code) => Ok((None, Some(code))),
        Ok(namespace) => {
            let mut rows: Vec<(String, Py<PyAny>)> = Vec::with_capacity(namespace.entries.len());
            for (name, value) in namespace.entries {
                rows.push((name, object_of(py, &host, value)?));
            }
            Ok((Some(rows), None))
        }
    }
}

/// Parse CLI arguments into a `CliNamespace`; help, version and errors print and set the exit code.
#[pyfunction]
#[pyo3(signature = (argv, *, use_color))]
pub(crate) fn parse_cli_invocation<'py>(
    py: Python<'py>,
    argv: Vec<Bound<'py, PyString>>,
    use_color: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let args: Bound<'py, PyAny> = new_cli_namespace(py)?;
    let defaults: Bound<'py, PyList> = py
        .get_type::<PyList>()
        .call1((args.getattr("__dict__")?.call_method0("items")?,))?
        .downcast_into::<PyList>()?;
    let columns: i64 = py
        .import("shutil")?
        .call_method0("get_terminal_size")?
        .getattr("columns")?
        .extract()?;
    let settings: ArgvSettings = ArgvSettings { columns, use_color };
    let (rows, exit_code): ParsedRows = parse_cli_arguments(&argv, &defaults, settings)?;
    let Some(rows) = rows else {
        return new_record_with(
            py,
            RecordId::ParsedCliInvocation,
            &[
                ("args", py.None().into_bound(py)),
                ("exit_code", exit_code.into_bound_py_any(py)?),
            ],
        );
    };
    for (name, value) in rows {
        args.setattr(name.as_str(), value)?;
    }
    new_record_with(
        py,
        RecordId::ParsedCliInvocation,
        &[("args", args), ("exit_code", py.None().into_bound(py))],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_cli_invocation, module)?)?;
    Ok(())
}
