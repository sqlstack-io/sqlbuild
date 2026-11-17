//! Python calls, errors and request facts shared by the `sqb diff` command phases.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_cli::diff_report::models::DiffRequestFacts;
use sqlbuild_host::entries::models::HostId;
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

/// `name(**pairs)` with keyword arguments.
pub(crate) fn call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, &Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(*name, *value)?;
    }
    callable.call((), Some(&keywords))
}

pub(crate) fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

pub(crate) fn object<'py, T: pyo3::IntoPyObjectExt<'py>>(
    py: Python<'py>,
    value: T,
) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

/// `class(message, code=code, help=help)` from the CLI exceptions module.
pub(crate) fn cli_error(py: Python<'_>, class: ErrorId, error: &DiffUserError) -> PyErr {
    new_error(
        py,
        class,
        &error.message,
        (Some(&error.code), error.help.as_deref()),
    )
}

/// The `message`, `code` and `help` of a raised `CliUserError`.
pub(crate) fn user_error_parts(error: &Bound<'_, PyAny>) -> PyResult<DiffUserError> {
    let help: Bound<'_, PyAny> = native_field(error, "help")?;
    Ok(DiffUserError {
        message: native_field(error, "message")?.extract()?,
        code: native_field(error, "code")?.str()?.extract()?,
        help: if help.is_none() {
            None
        } else {
            Some(help.extract()?)
        },
    })
}

/// `error` re-raised as `class` with `error` as its cause.
pub(crate) fn caused(py: Python<'_>, replacement: PyErr, cause: PyErr) -> PyErr {
    replacement.set_cause(py, Some(cause));
    replacement
}

pub(crate) fn matches_class(
    py: Python<'_>,
    error: &PyErr,
    class: impl Into<HostId>,
) -> PyResult<bool> {
    error.value(py).is_instance(&host_entry(py, class)?)
}

/// Whether `error` is an instance of the native error class `class`.
pub(crate) fn matches_native(py: Python<'_>, error: &PyErr, class: ErrorId) -> PyResult<bool> {
    error.value(py).is_instance(&error_class(py, class)?)
}

/// Whether `error` is an `Exception` rather than a `BaseException` such as an interrupt.
pub(crate) fn is_exception(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyException>(py)
}

pub(crate) fn error_text(py: Python<'_>, error: &PyErr) -> PyResult<String> {
    error.value(py).str()?.extract()
}

/// Write one line to `sys.stderr`, as `print(..., file=sys.stderr)` does.
pub(crate) fn progress_line(py: Python<'_>, line: &str) -> PyResult<()> {
    imported(py, "sys", "stderr")?.call_method1("write", (format!("{line}\n"),))?;
    Ok(())
}

fn optional_text(value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

fn texts(value: Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut items: Vec<String> = Vec::new();
    for item in value.try_iter()? {
        items.push(item?.str()?.extract()?);
    }
    Ok(items)
}

/// The facts of one `DiffCommandRequest` that validation and the size guard read.
pub(crate) fn request_facts(request: &Bound<'_, PyAny>) -> PyResult<DiffRequestFacts> {
    let py: Python<'_> = request.py();
    let attribute = |name: &str| native_field(request, name);
    let present = |name: &str| -> PyResult<bool> { Ok(!attribute(name)?.is_none()) };
    let number = |name: &str| -> PyResult<Option<i64>> { attribute(name)?.extract() };
    let flag = |name: &str| -> PyResult<bool> { attribute(name)?.is_truthy() };
    let variables: Bound<'_, PyAny> = attribute("cli_vars")?;
    let cli_vars_json: Option<String> = if variables.is_truthy()? {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("sort_keys", true)?;
        Some(
            imported(py, "json", "dumps")?
                .call((variables,), Some(&keywords))?
                .extract()?,
        )
    } else {
        None
    };
    Ok(DiffRequestFacts {
        from_name: optional_text(attribute("from_name")?)?,
        to_name: optional_text(attribute("to_name")?)?,
        full: flag("full")?,
        schema_only: flag("schema_only")?,
        bounded: optional_text(attribute("bounded")?)?,
        max_column_examples: number("max_column_examples")?,
        max_row_only_examples: number("max_row_only_examples")?,
        sample_rows: number("sample_rows")?,
        sample_seed: number("sample_seed")?,
        exhaustive: flag("exhaustive")?,
        max_models: number("max_models")?,
        max_columns: number("max_columns")?,
        max_value_length: number("max_value_length")?,
        full_example_values: flag("full_example_values")?,
        suppress_example_values: flag("suppress_example_values")?,
        select: texts(attribute("select")?)?,
        exclude: texts(attribute("exclude")?)?,
        selected_target: optional_text(attribute("selected_target")?)?,
        left_query: present("left_query")?,
        left_query_file: present("left_query_file")?,
        right_query: present("right_query")?,
        right_query_file: present("right_query_file")?,
        left_label: optional_text(attribute("left_label")?)?,
        right_label: optional_text(attribute("right_label")?)?,
        unique_key_override: texts(attribute("unique_key_override")?)?,
        unkeyed: flag("unkeyed")?,
        excluded_columns_override: texts(attribute("excluded_columns_override")?)?,
        tolerance_overrides: texts(attribute("tolerance_overrides")?)?,
        cli_vars_json,
        project_dir: optional_text(attribute("project_dir")?)?,
    })
}
